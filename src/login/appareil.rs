// ══════════════════════════════════════════════════════════════════
// login/appareil.rs — Flux d'autorisation d'appareil ("device flow",
// meme principe que GitHub CLI / Docker Desktop) pour vex-cloudsync.
//
// Objectif : un appareil desktop (sans navigateur/session propre) obtient
// un jeton d'acces longue duree en faisant approuver sa demande par
// l'utilisateur, deja connecte, via une page web sur CE serveur.
//
// Etapes :
//   1. POST /api/appareil/demander       (appareil, sans auth) -> code
//   2. GET  /autoriser-appareil?code=... (navigateur, auth cookie requise)
//   3. POST /api/appareil/approuver      (navigateur, auth cookie requise)
//   4. GET  /api/appareil/statut?code=.. (appareil, sans auth, poll)
//
// SECURITE :
//   - Le jeton brut n'est JAMAIS renvoye a l'etape 3 (cote navigateur) --
//     uniquement recupere par l'appareil lui-meme a l'etape 4.
//   - `jeton_brut` n'est stocke qu'entre l'approbation et la premiere
//     recuperation reussie, puis efface (seul `jeton_hash` subsiste).
//   - Un code expire au bout de 10 minutes s'il n'a pas ete approuve.
//   - `/api/appareil/approuver` s'appuie sur le cookie `connexion_cookie`
//     (HttpOnly + SameSite=Strict, voir login.rs) pour l'auth ET la
//     protection CSRF -- SameSite=Strict empeche ce cookie d'etre envoye
//     depuis un site tiers, donc un site malveillant ne peut pas forcer
//     une approbation a l'insu de l'utilisateur.
//   - Risque residuel connu et INHERENT a ce type de flux ("device code
//     phishing", deja exploite en pratique contre les flux OAuth device
//     de Microsoft/Google) : un attaquant genere lui-meme un code via
//     /api/appareil/demander puis piege la victime (deja connectee) pour
//     qu'elle clique "Autoriser" sur CE code -- la victime donnerait alors
//     un jeton d'acces complet a l'attaquant. Mitige au mieux cote page
//     d'approbation par un avertissement explicite + affichage du code
//     (a comparer visuellement avec celui affiche par l'app desktop) mais
//     ne peut pas etre elimine a 100% par du code seul : depend de la
//     vigilance de l'utilisateur, comme pour tous les flux "device code".
//   - Revocation : `statut='revoque'` (voir api_revoquer) -- a l'avenir,
//     toute verification du jeton DOIT rejeter un statut != 'approuve'.
//   - PAS ENCORE FAIT (voir PLAN-INSTALLATION-1-CLIC.md) : utiliser ce
//     jeton pour authentifier les appels fchier existants -- cette
//     premiere version ne fait que l'emission/l'approbation/la revocation
//     du jeton.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, verifier_connexion, DbPool};
use chrono::{NaiveDateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use tiny_http::{Request, Response};

const EXPIRATION_MINUTES: i64 = 10;
const LONGUEUR_CODE: usize = 10;
const LONGUEUR_JETON: usize = 48;

pub fn handle_request(mut request: Request, pool: &DbPool, remote_ip: &str) {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let query = parser_query(url.split('?').nth(1).unwrap_or(""));
    let cookie_val = extraire_cookie(request.headers(), "connexion_cookie");
    let user_agent = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "user-agent")
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    let methode = request.method().as_str().to_string();

    let reponse = match (methode.as_str(), path.as_str()) {
        ("POST", "/api/appareil/demander") => api_demander(&mut request, pool),
        ("GET", "/autoriser-appareil") => {
            page_autorisation(pool, &query, &cookie_val, remote_ip, &user_agent)
        }
        ("POST", "/api/appareil/approuver") => {
            api_approuver(&mut request, pool, &cookie_val, remote_ip, &user_agent)
        }
        ("GET", "/api/appareil/statut") => api_statut(pool, &query),
        ("GET", "/api/appareil/liste") => api_liste(pool, &cookie_val, remote_ip, &user_agent),
        ("POST", "/api/appareil/revoquer") => {
            api_revoquer(&mut request, pool, &cookie_val, remote_ip, &user_agent)
        }
        ("GET", "/api/appareil/telecharger") => {
            telecharger_bundle(pool, &request, &cookie_val, remote_ip, &user_agent)
        }
        _ => reponse_json(json!({"success": false, "error": "route inconnue"}), 404),
    };
    let _ = request.respond(reponse);
}

// ══════════════════════════════════════════════════════════════════
// ÉTAPE 1 — POST /api/appareil/demander (aucune auth : l'appareil n'a
// pas encore de session, c'est justement le but de ce flux)
// ══════════════════════════════════════════════════════════════════
fn api_demander(request: &mut Request, pool: &DbPool) -> Response<std::io::Cursor<Vec<u8>>> {
    let corps = lire_body(request);
    let params = parser_query(&corps);
    let nom_appareil = params
        .get("nom_appareil")
        .cloned()
        .unwrap_or_else(|| "Appareil inconnu".to_string());
    // Longueur raisonnable : evite d'accepter un nom demesure dans la DB.
    let nom_appareil: String = nom_appareil.chars().take(191).collect();

    let code = match generer_aleatoire(LONGUEUR_CODE) {
        Ok(c) => c,
        Err(_) => return reponse_json(json!({"success": false, "error": "generation impossible"}), 500),
    };

    let id = inserer_ou_modifier(
        pool,
        "appareil_jetons",
        &[
            ("code", mysql::Value::from(code.as_str())),
            ("statut", mysql::Value::from("en_attente")),
            ("nom_appareil", mysql::Value::from(nom_appareil.as_str())),
        ],
        &[],
    );
    if id < 0 {
        return reponse_json(json!({"success": false, "error": "insertion impossible"}), 500);
    }

    reponse_json(json!({"success": true, "code": code}), 200)
}

// ══════════════════════════════════════════════════════════════════
// ÉTAPE 2 — GET /autoriser-appareil?code=... (auth cookie requise)
// ══════════════════════════════════════════════════════════════════
fn page_autorisation(
    pool: &DbPool,
    query: &HashMap<String, String>,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let code = query.get("code").cloned().unwrap_or_default();

    if verifier_connexion(pool, cookie_val, remote_ip, user_agent).is_none() {
        // Pas connecte : on renvoie vers la page de login, avec un retour
        // vers cette meme page une fois connecte.
        let retour = format!("/autoriser-appareil?code={}", code);
        return Response::from_string("")
            .with_status_code(303)
            .with_header(
                tiny_http::Header::from_bytes(
                    "Location",
                    format!("/login/login?retour={}", url_encode(&retour)),
                )
                .unwrap(),
            );
    }

    let ligne = ligne_par_code(pool, &code);
    let (titre, corps): (&str, String) = match &ligne {
        None => ("Code invalide", "Ce code d'autorisation n'existe pas ou a expiré.".to_string()),
        Some(l) if code_expire(l) => ("Code expiré", "Cette demande a expiré. Relance la connexion depuis l'appareil.".to_string()),
        Some(l) if l.get("statut").and_then(|v| v.as_str()) != Some("en_attente") => {
            ("Déjà traité", "Cette demande a déjà été traitée.".to_string())
        }
        Some(l) => {
            let nom = l.get("nom_appareil").and_then(|v| v.as_str()).unwrap_or("Appareil inconnu");
            (
                "Autoriser cet appareil ?",
                format!(
                    "<p>L'appareil <strong>{}</strong> demande à accéder à tes fichiers VEX \
                     (lecture, écriture, suppression -- comme si tu étais connecté dessus).</p>\
                     <p style=\"background:#1c2128;border:1px solid #2a2f3a;border-radius:8px;padding:10px 14px;\
                     font-size:.8rem;color:#9aa1ad\">Code affiché sur l'appareil : \
                     <strong style=\"color:#e7e9ee;letter-spacing:1px\">{}</strong> — vérifie qu'il correspond \
                     bien à ce qui s'affiche sur l'appareil que tu essaies de connecter.</p>\
                     <p style=\"color:#ffb347;font-size:.8rem\">⚠ N'autorise QUE si tu viens toi-même de lancer \
                     VEX Cloud Sync sur un appareil que tu contrôles. Si tu n'as rien lancé, ou si ce lien t'a été \
                     envoyé par quelqu'un d'autre, clique Refuser.</p>\
                     <div style=\"display:flex;gap:10px;margin-top:20px\">\
                     <button onclick=\"repondre('oui')\" style=\"flex:1;padding:12px;background:#4caf50;color:#fff;border:none;border-radius:8px;font-weight:700;cursor:pointer\">Autoriser</button>\
                     <button onclick=\"repondre('non')\" style=\"flex:1;padding:12px;background:#2a2f3a;color:#fff;border:none;border-radius:8px;font-weight:700;cursor:pointer\">Refuser</button>\
                     </div>\
                     <p id=\"resultat\" style=\"margin-top:16px;font-size:.85rem\"></p>",
                    escaper_html(nom),
                    escaper_html(&code)
                ),
            )
        }
    };

    let html = format!(
        r#"<!DOCTYPE html><html lang="fr"><head><meta charset="UTF-8">
<title>Autoriser un appareil — VEX</title>
<style>
body {{ font-family:-apple-system,sans-serif; background:#0f1115; color:#e7e9ee; display:flex;
       justify-content:center; padding:60px 16px; margin:0; }}
.carte {{ max-width:440px; background:#171a21; border:1px solid #2a2f3a; border-radius:14px; padding:28px; }}
h1 {{ font-size:1.1rem; margin:0 0 14px; }}
p {{ font-size:.9rem; line-height:1.6; color:#c7cbd4; }}
</style></head><body>
<div class="carte"><h1>{}</h1>{}</div>
<script>
function repondre(decision) {{
  fetch('/api/appareil/approuver', {{
    method:'POST', headers:{{'Content-Type':'application/x-www-form-urlencoded'}}, credentials:'include',
    body:'code={}&decision=' + decision,
  }}).then(r => r.json()).then(d => {{
    document.getElementById('resultat').textContent = d.success
      ? (decision === 'oui' ? 'Appareil autorisé. Tu peux fermer cette page.' : 'Refusé. Tu peux fermer cette page.')
      : (d.error || 'Erreur.');
  }});
}}
</script>
</body></html>"#,
        titre, corps, url_encode(&code)
    );

    Response::from_string(html).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
    )
}

// ══════════════════════════════════════════════════════════════════
// ÉTAPE 3 — POST /api/appareil/approuver (auth cookie requise)
// ══════════════════════════════════════════════════════════════════
fn api_approuver(
    request: &mut Request,
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        return reponse_json(json!({"success": false, "error": "non authentifié"}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    let corps = lire_body(request);
    let params = parser_query(&corps);
    let code = params.get("code").cloned().unwrap_or_default();
    let decision = params.get("decision").cloned().unwrap_or_default();

    let Some(ligne) = ligne_par_code(pool, &code) else {
        return reponse_json(json!({"success": false, "error": "code introuvable"}), 404);
    };
    if code_expire(&ligne) {
        return reponse_json(json!({"success": false, "error": "code expiré"}), 410);
    }
    if ligne.get("statut").and_then(|v| v.as_str()) != Some("en_attente") {
        return reponse_json(json!({"success": false, "error": "déjà traité"}), 409);
    }

    if decision == "oui" {
        let jeton_brut = match generer_aleatoire(LONGUEUR_JETON) {
            Ok(j) => j,
            Err(_) => return reponse_json(json!({"success": false, "error": "génération impossible"}), 500),
        };
        let jeton_hash = crate::appeldb::hasher_jeton_appareil(&jeton_brut);
        inserer_ou_modifier(
            pool,
            "appareil_jetons",
            &[
                ("jeton_brut", mysql::Value::from(jeton_brut.as_str())),
                ("jeton_hash", mysql::Value::from(jeton_hash.as_str())),
                ("user_id", mysql::Value::from(user_id)),
                ("statut", mysql::Value::from("approuve")),
            ],
            &[("code", mysql::Value::from(code.as_str()))],
        );
    } else {
        inserer_ou_modifier(
            pool,
            "appareil_jetons",
            &[("statut", mysql::Value::from("refuse"))],
            &[("code", mysql::Value::from(code.as_str()))],
        );
    }

    reponse_json(json!({"success": true}), 200)
}

// ══════════════════════════════════════════════════════════════════
// ÉTAPE 4 — GET /api/appareil/statut?code=... (aucune auth : c'est
// l'appareil lui-même qui interroge, il n'a pas encore de session)
// ══════════════════════════════════════════════════════════════════
fn api_statut(pool: &DbPool, query: &HashMap<String, String>) -> Response<std::io::Cursor<Vec<u8>>> {
    let code = query.get("code").cloned().unwrap_or_default();
    let Some(ligne) = ligne_par_code(pool, &code) else {
        return reponse_json(json!({"statut": "introuvable"}), 200);
    };
    if code_expire(&ligne) && ligne.get("statut").and_then(|v| v.as_str()) == Some("en_attente") {
        return reponse_json(json!({"statut": "expire"}), 200);
    }

    match ligne.get("statut").and_then(|v| v.as_str()) {
        Some("approuve") => {
            let jeton_brut = ligne.get("jeton_brut").and_then(|v| v.as_str()).unwrap_or("");
            if jeton_brut.is_empty() {
                // Deja recupere par un poll precedent -- pas renvoye deux fois.
                reponse_json(json!({"statut": "deja_recupere"}), 200)
            } else {
                let reponse = json!({"statut": "approuve", "jeton": jeton_brut});
                // Efface le jeton brut immediatement : une seule livraison.
                inserer_ou_modifier(
                    pool,
                    "appareil_jetons",
                    &[("jeton_brut", mysql::Value::NULL)],
                    &[("code", mysql::Value::from(code.as_str()))],
                );
                reponse_json(reponse, 200)
            }
        }
        Some("refuse") => reponse_json(json!({"statut": "refuse"}), 200),
        _ => reponse_json(json!({"statut": "en_attente"}), 200),
    }
}

// ══════════════════════════════════════════════════════════════════
// GESTION — liste et révocation des appareils autorisés (auth cookie
// requise). C'est ce qui permet de couper l'accès d'un appareil perdu
// ou compromis sans devoir changer son mot de passe.
// ══════════════════════════════════════════════════════════════════
fn api_liste(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        return reponse_json(json!({"success": false, "error": "non authentifié"}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    // WHERE user_id = ? exclut déjà naturellement les codes en_attente/refusé
    // (user_id n'est renseigné qu'au moment de l'approbation).
    let lignes = selectionner(
        pool,
        "appareil_jetons",
        &[("user_id", mysql::Value::from(user_id))],
        &["code", "nom_appareil", "statut", "created_at"],
        Some("created_at DESC"),
        None,
    );

    let appareils: Vec<Value> = lignes
        .into_iter()
        .map(|l| {
            json!({
                "code": l.get("code").cloned().unwrap_or(Value::Null),
                "nom_appareil": l.get("nom_appareil").cloned().unwrap_or(Value::Null),
                "statut": l.get("statut").cloned().unwrap_or(Value::Null),
                "created_at": l.get("created_at").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();

    reponse_json(json!({"success": true, "appareils": appareils}), 200)
}

fn api_revoquer(
    request: &mut Request,
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        return reponse_json(json!({"success": false, "error": "non authentifié"}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    let corps = lire_body(request);
    let params = parser_query(&corps);
    let code = params.get("code").cloned().unwrap_or_default();

    let Some(ligne) = ligne_par_code(pool, &code) else {
        return reponse_json(json!({"success": false, "error": "code introuvable"}), 404);
    };
    // Vérification de propriété : un utilisateur ne peut révoquer que SES
    // propres appareils, jamais ceux d'un autre en devinant/énumérant un code.
    if ligne.get("user_id").and_then(|v| v.as_i64()) != Some(user_id) {
        return reponse_json(json!({"success": false, "error": "non autorisé"}), 403);
    }

    inserer_ou_modifier(
        pool,
        "appareil_jetons",
        &[("statut", mysql::Value::from("revoque")), ("jeton_brut", mysql::Value::NULL)],
        &[("code", mysql::Value::from(code.as_str()))],
    );

    reponse_json(json!({"success": true}), 200)
}

// ══════════════════════════════════════════════════════════════════
// TÉLÉCHARGEMENT — bundle vex-cloudsync.exe + config.json (auth cookie
// requise). Un seul exécutable générique deployé une fois sur le
// serveur (voir static/downloads/vex-cloudsync.exe) -- pas de
// recompilation par utilisateur, seul config.json est genere a la volee
// avec l'URL publique reelle (deduite du Host de la requete).
// ══════════════════════════════════════════════════════════════════
const CHEMIN_EXE_CLOUDSYNC: &str = "static/downloads/vex-cloudsync.exe";

fn telecharger_bundle(
    pool: &DbPool,
    request: &Request,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if verifier_connexion(pool, cookie_val, remote_ip, user_agent).is_none() {
        return reponse_json(json!({"success": false, "error": "non authentifié"}), 401);
    }

    let exe = match std::fs::read(CHEMIN_EXE_CLOUDSYNC) {
        Ok(o) => o,
        Err(_) => {
            return reponse_json(
                json!({"success": false, "error": "vex-cloudsync.exe indisponible sur le serveur"}),
                404,
            )
        }
    };

    let host = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "host")
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default();
    if host.is_empty() {
        return reponse_json(json!({"success": false, "error": "en-tête Host manquant"}), 400);
    }
    // Le site n'est servi qu'en HTTPS en production (reverse proxy) --
    // voir remarque remote_ip dans fchier.rs sur l'architecture derriere
    // Apache. On construit donc l'URL publique en HTTPS.
    let base_url = format!("https://{}", host);
    let config = json!({"base_url": base_url}).to_string();

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        if zip.start_file("vex-cloudsync.exe", options).is_err()
            || std::io::Write::write_all(&mut zip, &exe).is_err()
            || zip.start_file("config.json", options).is_err()
            || std::io::Write::write_all(&mut zip, config.as_bytes()).is_err()
            || zip.finish().is_err()
        {
            return reponse_json(json!({"success": false, "error": "échec de génération de l'archive"}), 500);
        }
    }

    Response::from_data(buf)
        .with_header(tiny_http::Header::from_bytes("Content-Type", "application/zip").unwrap())
        .with_header(
            tiny_http::Header::from_bytes(
                "Content-Disposition",
                "attachment; filename=\"vex-cloudsync.zip\"",
            )
            .unwrap(),
        )
}

// ══════════════════════════════════════════════════════════════════
// Aides
// ══════════════════════════════════════════════════════════════════
fn ligne_par_code(pool: &DbPool, code: &str) -> Option<HashMap<String, Value>> {
    if code.is_empty() {
        return None;
    }
    selectionner(
        pool,
        "appareil_jetons",
        &[("code", mysql::Value::from(code))],
        &["code", "jeton_brut", "user_id", "statut", "nom_appareil", "created_at"],
        None,
        Some(1),
    )
    .into_iter()
    .next()
}

fn code_expire(ligne: &HashMap<String, Value>) -> bool {
    let Some(created) = ligne.get("created_at").and_then(|v| v.as_str()) else { return true };
    let Some(dt) = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"]
        .iter()
        .find_map(|fmt| NaiveDateTime::parse_from_str(created, fmt).ok())
    else {
        return true;
    };
    Utc::now().naive_utc().signed_duration_since(dt) > chrono::Duration::minutes(EXPIRATION_MINUTES)
}

/// CSPRNG via getrandom, alphabet alphanumerique -- meme principe que
/// `autologin.rs::generer_token_brut` (rejet uniforme anti-biais modulo).
fn generer_aleatoire(longueur: usize) -> Result<String, getrandom::Error> {
    let charset = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let len = charset.len() as u8;
    let limit = (255u8 / len) * len;
    let mut result = String::with_capacity(longueur);
    let mut buf = vec![0u8; longueur * 2];
    getrandom::getrandom(&mut buf)?;
    let mut i = 0;
    while result.len() < longueur {
        if i >= buf.len() {
            let mut more = vec![0u8; longueur];
            getrandom::getrandom(&mut more)?;
            buf.extend(more);
        }
        let b = buf[i];
        i += 1;
        if b < limit {
            result.push(charset[(b % len) as usize] as char);
        }
    }
    Ok(result)
}

fn extraire_cookie(headers: &[tiny_http::Header], name: &str) -> String {
    headers
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "cookie")
        .and_then(|h| {
            h.value
                .as_str()
                .split(';')
                .map(|p| p.trim())
                .find(|p| p.starts_with(&format!("{}=", name)))
                .and_then(|p| p.splitn(2, '=').nth(1))
        })
        .unwrap_or_default()
        .to_string()
}

fn lire_body(request: &mut Request) -> String {
    let mut body = String::new();
    let mut limited = request.as_reader().take(8192);
    let _ = Read::read_to_string(&mut limited, &mut body);
    body
}

fn parser_query(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for part in s.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            map.insert(url_decode(k), url_decode(v));
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn escaper_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn reponse_json(val: Value, status: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(val.to_string())
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
        )
}
