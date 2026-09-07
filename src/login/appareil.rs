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

use super::notice_cloudsync;
use crate::appeldb::{inserer_ou_modifier, selectionner, verifier_connexion, DbPool};
use chrono::{NaiveDateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
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
    let bearer = request
        .headers()
        .iter()
        .find(|h| h.field.as_str().to_ascii_lowercase() == "authorization")
        .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    let host = crate::access_control::get_header(&request, "Host");
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
        ("GET", "/api/appareil/telecharger") | ("HEAD", "/api/appareil/telecharger") => {
            let zip = query.get("zip").is_some();
            telecharger_bundle(pool, &methode, &cookie_val, &bearer, remote_ip, &user_agent, zip)
        }
        ("GET", "/install.ps1") => script_installation(&host),
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
    use crate::i18n::{t, Cle};
    let code = query.get("code").cloned().unwrap_or_default();

    let Some(session) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
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
    };
    let user_id = session.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let theme = crate::function::get_theme_attr(pool, user_id);
    let langue = crate::function::get_user_language(pool, Some(user_id), None, None);

    let ligne = ligne_par_code(pool, &code);
    let (titre, corps): (&str, String) = match &ligne {
        None => (t(&langue, Cle::AppareilCodeInvalideTitre), t(&langue, Cle::AppareilCodeInvalideTexte).to_string()),
        Some(l) if code_expire(l) => (t(&langue, Cle::AppareilCodeExpireTitre), t(&langue, Cle::AppareilCodeExpireTexte).to_string()),
        Some(l) if l.get("statut").and_then(|v| v.as_str()) != Some("en_attente") => {
            (t(&langue, Cle::AppareilDejaTraiteTitre), t(&langue, Cle::AppareilDejaTraiteTexte).to_string())
        }
        Some(l) => {
            let nom = l.get("nom_appareil").and_then(|v| v.as_str()).unwrap_or("Appareil inconnu");
            let demande = t(&langue, Cle::AppareilDemandeAcces)
                .replace("{nom}", &format!("<strong>{}</strong>", escaper_html(nom)));
            (
                t(&langue, Cle::AppareilAutoriserTitre),
                format!(
                    "<p>{demande}</p>\
                     <p style=\"background:var(--surface2);border:1px solid var(--border);border-radius:8px;padding:10px 14px;\
                     font-size:.8rem;color:var(--text-dim)\">{label} \
                     <strong style=\"color:var(--text);letter-spacing:1px\">{code}</strong> — {verifie}</p>\
                     <p style=\"color:var(--orange);font-size:.8rem\">⚠ {avertissement}</p>\
                     <div style=\"display:flex;gap:10px;margin-top:20px\">\
                     <button onclick=\"repondre('oui')\" style=\"flex:1;padding:12px;background:var(--accent);color:#fff;border:none;border-radius:8px;font-weight:700;cursor:pointer\">{bouton_autoriser}</button>\
                     <button onclick=\"repondre('non')\" style=\"flex:1;padding:12px;background:var(--surface2);color:var(--text);border:1px solid var(--border);border-radius:8px;font-weight:700;cursor:pointer\">{bouton_refuser}</button>\
                     </div>\
                     <p id=\"resultat\" style=\"margin-top:16px;font-size:.85rem;color:var(--text)\"></p>",
                    demande = demande,
                    label = t(&langue, Cle::AppareilCodeAfficheLabel),
                    code = escaper_html(&code),
                    verifie = t(&langue, Cle::AppareilVerifieCorrespond),
                    avertissement = t(&langue, Cle::AppareilAvertissement),
                    bouton_autoriser = t(&langue, Cle::AppareilBoutonAutoriser),
                    bouton_refuser = t(&langue, Cle::AppareilBoutonRefuser),
                ),
            )
        }
    };

    let html = format!(
        r#"<!DOCTYPE html><html lang="{langue}" data-theme="{theme}"><head><meta charset="UTF-8">
<title>{titre_page}</title>
<link rel="stylesheet" href="/static/css/theme.css">
<style>
body {{ font-family:-apple-system,sans-serif; background:var(--bg); color:var(--text); display:flex;
       justify-content:center; padding:60px 16px; margin:0; }}
.carte {{ max-width:440px; background:var(--panel); border:1px solid var(--panel-border); border-radius:14px; padding:28px; }}
h1 {{ font-size:1.1rem; margin:0 0 14px; }}
p {{ font-size:.9rem; line-height:1.6; color:var(--text-dim); }}
</style></head><body>
<div class="carte"><h1>{titre}</h1>{corps}</div>
<script>
function repondre(decision) {{
  fetch('/api/appareil/approuver', {{
    method:'POST', headers:{{'Content-Type':'application/x-www-form-urlencoded'}}, credentials:'include',
    body:'code={code_enc}&decision=' + decision,
  }}).then(r => r.json()).then(d => {{
    document.getElementById('resultat').textContent = d.success
      ? (decision === 'oui' ? '{resultat_autorise}' : '{resultat_refuse}')
      : (d.error || '{erreur_generique}');
  }});
}}
</script>
</body></html>"#,
        langue = langue,
        theme = theme,
        titre_page = t(&langue, Cle::AppareilTitrePage),
        titre = titre,
        corps = corps,
        code_enc = url_encode(&code),
        resultat_autorise = t(&langue, Cle::AppareilResultatAutorise).replace('\'', "\\'"),
        resultat_refuse = t(&langue, Cle::AppareilResultatRefuse).replace('\'', "\\'"),
        erreur_generique = t(&langue, Cle::AppareilErreurGenerique).replace('\'', "\\'"),
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
    use crate::i18n::{t, Cle};
    let accept_lang = crate::access_control::get_header(request, "Accept-Language");
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        let langue = crate::function::get_user_language(pool, None, None, Some(&accept_lang));
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurNonAuthentifie)}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let langue = crate::function::get_user_language(pool, Some(user_id), None, Some(&accept_lang));

    let corps = lire_body(request);
    let params = parser_query(&corps);
    let code = params.get("code").cloned().unwrap_or_default();
    let decision = params.get("decision").cloned().unwrap_or_default();

    let Some(ligne) = ligne_par_code(pool, &code) else {
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurCodeIntrouvable)}), 404);
    };
    if code_expire(&ligne) {
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilCodeExpireTitre)}), 410);
    }
    if ligne.get("statut").and_then(|v| v.as_str()) != Some("en_attente") {
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilDejaTraiteTitre)}), 409);
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
    use crate::i18n::{t, Cle};
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        let langue = crate::function::get_user_language(pool, None, None, None);
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurNonAuthentifie)}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);

    // WHERE user_id = ? exclut déjà naturellement les codes en_attente/refusé
    // (user_id n'est renseigné qu'au moment de l'approbation). statut =
    // 'approuve' en plus : la page affiche "Appareils autorisés", donc un
    // appareil revoque n'a plus sa place dans cette liste -- BUG CONSTATE EN
    // PRATIQUE, il y restait indefiniment.
    let lignes = selectionner(
        pool,
        "appareil_jetons",
        &[("user_id", mysql::Value::from(user_id)), ("statut", mysql::Value::from("approuve"))],
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
    use crate::i18n::{t, Cle};
    let Some(user_info) = verifier_connexion(pool, cookie_val, remote_ip, user_agent) else {
        let langue = crate::function::get_user_language(pool, None, None, None);
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurNonAuthentifie)}), 401);
    };
    let user_id = user_info.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let langue = crate::function::get_user_language(pool, Some(user_id), None, None);

    let corps = lire_body(request);
    let params = parser_query(&corps);
    let code = params.get("code").cloned().unwrap_or_default();

    let Some(ligne) = ligne_par_code(pool, &code) else {
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurCodeIntrouvable)}), 404);
    };
    // Vérification de propriété : un utilisateur ne peut révoquer que SES
    // propres appareils, jamais ceux d'un autre en devinant/énumérant un code.
    if ligne.get("user_id").and_then(|v| v.as_i64()) != Some(user_id) {
        return reponse_json(json!({"success": false, "error": t(&langue, Cle::AppareilErreurNonAutorise)}), 403);
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
const CHEMIN_DESINSTALLER: &str = "static/downloads/desinstaller.exe";
// Le zip (exe + notice + langue.txt) n'est PAS un fichier statique : il est
// genere a la volee a chaque telechargement (voir plus bas), pour inclure la
// notice traduite dans la langue de COMPTE de l'utilisateur qui telecharge
// (function::get_user_language) -- deux utilisateurs avec des langues
// differentes ne doivent pas recevoir le meme zip. install.ps1 continue de
// recuperer l'exe brut directement (pas besoin de notice pour un script
// automatise).

// L'exe hebergee ici est un fichier STATIQUE : deja pre-configuree (essaie
// plusieurs URLs connues au demarrage, voir BASE_URL_CANDIDATS dans
// clients/vex-cloudsync/src/main.rs) et deja signee EN LOCAL par le
// developpeur avant d'etre placee ici. Le serveur ne patche plus rien et
// ne signe plus rien a la volee -- la cle privee de signature ne doit
// JAMAIS se trouver sur ce serveur (decide explicitement avec
// l'utilisateur apres avoir pese le compromis de securite).
// BUGS CORRIGES (constates en pratique, l'un cachait l'autre) :
//  1. Sans "Accept-Ranges: none", Edge/Chrome declenchent un
//     telechargement SEGMENTE en parallele pour les fichiers de cette
//     taille -- tiny_http ignorait Range et renvoyait l'exe COMPLET a
//     chaque segment.
//  2. tiny_http bascule AUTOMATIQUEMENT en Transfer-Encoding: chunked
//     (RFC 7230) des qu'une reponse depasse son "chunked_threshold" par
//     defaut (32 768 octets -- voir tiny_http::Response::chunked_threshold),
//     meme quand la taille exacte est deja connue (from_data). Un exe de
//     plusieurs Mo passe donc systematiquement en chunked. Ce mode passe
//     mal a travers Apache (reverse proxy devant ce serveur) pour un
//     transfert binaire de cette taille -- constate : fichier final a
//     0 Ko cote navigateur alors que le flux chunked recu (verifie via un
//     client de test independant) contenait bien tous les octets. Fixer
//     un seuil de bascule chunked superieur a la taille du fichier force
//     un Content-Length classique, fiable a travers n'importe quel proxy.
fn telecharger_bundle(
    pool: &DbPool,
    methode: &str,
    cookie_val: &str,
    bearer: &str,
    remote_ip: &str,
    user_agent: &str,
    zip: bool,
) -> Response<std::io::Cursor<Vec<u8>>> {
    // Auth par cookie (bouton "Telecharger" sur le site, session navigateur)
    // OU par jeton d'appareil approuve (script d'installation / client
    // desktop sans session navigateur, voir script_installation ci-dessous
    // et fchier.rs::verifier_session pour le meme principe cote fichiers).
    // On garde le HashMap (pas juste un bool) : le zip a besoin de l'id
    // utilisateur pour lire sa langue de compte.
    let session_cookie = verifier_connexion(pool, cookie_val, remote_ip, user_agent);
    let session_bearer = crate::appeldb::verifier_jeton_appareil(pool, bearer);
    if session_cookie.is_none() && session_bearer.is_none() {
        return reponse_json(json!({"success": false, "error": "non authentifié"}), 401);
    }
    let user_id = session_cookie
        .as_ref()
        .or(session_bearer.as_ref())
        .and_then(|m| m.get("id"))
        .and_then(|v| v.as_i64());

    let (nom_fichier, content_type, corps) = if zip {
        let exe = match std::fs::read(CHEMIN_EXE_CLOUDSYNC) {
            Ok(o) => o,
            Err(_) => {
                return reponse_json(
                    json!({"success": false, "error": "vex-cloudsync.exe indisponible sur le serveur"}),
                    404,
                )
            }
        };
        // Optionnel : absent tant qu'on n'a pas encore deploye desinstaller.exe
        // sur cette machine -- le zip reste utilisable sans (juste sans
        // desinstalleur autonome, l'app elle-meme sait deja detecter une
        // installation existante, voir main.rs::deja_installe).
        let desinstaller = std::fs::read(CHEMIN_DESINSTALLER).ok();

        let langue = crate::function::get_user_language(pool, user_id, None, None);
        let (nom_notice, contenu_notice) = notice_cloudsync::contenu(&langue);

        let resultat = (|| -> zip::result::ZipResult<Vec<u8>> {
            let mut buf = std::io::Cursor::new(Vec::new());
            let mut writer = zip::ZipWriter::new(&mut buf);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer.start_file(nom_notice, options)?;
            writer.write_all(contenu_notice.as_bytes())?;
            writer.start_file("vex-cloudsync.exe", options)?;
            writer.write_all(&exe)?;
            if let Some(desinstaller) = &desinstaller {
                writer.start_file("desinstaller.exe", options)?;
                writer.write_all(desinstaller)?;
            }
            // Lue par l'app au tout premier lancement pour choisir sa langue
            // d'interface par defaut (voir clients/vex-cloudsync).
            writer.start_file("langue.txt", options)?;
            writer.write_all(langue.as_bytes())?;
            writer.finish()?;
            Ok(buf.into_inner())
        })();
        match resultat {
            Ok(v) => ("vex-cloudsync.zip", "application/zip", v),
            Err(_) => {
                return reponse_json(json!({"success": false, "error": "echec de generation du zip"}), 500)
            }
        }
    } else {
        let exe = match std::fs::read(CHEMIN_EXE_CLOUDSYNC) {
            Ok(o) => o,
            Err(_) => {
                return reponse_json(
                    json!({"success": false, "error": "vex-cloudsync.exe indisponible sur le serveur"}),
                    404,
                )
            }
        };
        ("vex-cloudsync.exe", "application/vnd.microsoft.portable-executable", exe)
    };
    let taille = corps.len();

    // HEAD : memes en-tetes (dont Content-Length reel), mais pas de corps
    // (les navigateurs l'utilisent parfois pour sonder la taille avant de
    // telecharger).
    let corps = if methode == "HEAD" { Vec::new() } else { corps };
    let mut reponse = Response::from_data(corps);
    if methode == "HEAD" {
        reponse = reponse.with_data(std::io::Cursor::new(Vec::new()), Some(taille));
    }

    reponse
        .with_chunked_threshold(usize::MAX)
        .with_header(tiny_http::Header::from_bytes("Content-Type", content_type).unwrap())
        .with_header(
            tiny_http::Header::from_bytes(
                "Content-Disposition",
                format!("attachment; filename=\"{nom_fichier}\""),
            )
            .unwrap(),
        )
        .with_header(tiny_http::Header::from_bytes("Accept-Ranges", "none").unwrap())
}

// ══════════════════════════════════════════════════════════════════
// SCRIPT D'INSTALLATION — GET /install.ps1 (aucune auth : c'est le point
// d'entree avant meme que l'appareil ait un jeton). Contourne le blocage
// "fichier rarement telecharge" du gestionnaire de telechargements des
// navigateurs (Firefox/Edge) qui, pour ce binaire encore peu diffuse,
// aboutissait a un fichier final de 0 Ko cote navigateur (voir
// telecharger_bundle ci-dessus pour le bug de transfert, deja corrige et
// distinct de celui-ci). En passant par `irm .../install.ps1 | iex`, le
// telechargement de l'exe se fait via Invoke-WebRequest (PowerShell), qui
// ne passe PAS par le gestionnaire de telechargements du navigateur et
// n'est donc pas soumis a cette verification de reputation cote navigateur.
// L'authentification se fait via le flux d'autorisation d'appareil deja
// en place (voir haut de fichier) : le script demande un code, ouvre la
// page d'approbation dans le navigateur (ou l'utilisateur est deja
// connecte via cookie), attend l'approbation, puis utilise le jeton
// obtenu en "Authorization: Bearer" pour l'appel de telechargement --
// meme mecanisme que celui deja utilise par fchier.rs::verifier_session
// pour les appels fichiers des clients desktop.
fn script_installation(host: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let base = format!("https://{}", host);
    let script = SCRIPT_INSTALLATION_TEMPLATE.replace("__BASE_URL__", &base);
    Response::from_data(script.into_bytes()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "text/plain; charset=utf-8").unwrap(),
    )
}

const SCRIPT_INSTALLATION_TEMPLATE: &str = r#"$ErrorActionPreference = 'Stop'
$base = '__BASE_URL__'

Write-Host "Connexion a VEX Cloud Sync..." -ForegroundColor Cyan

$demande = Invoke-RestMethod -Uri "$base/api/appareil/demander" -Method Post -Body @{ nom_appareil = $env:COMPUTERNAME }
if (-not $demande.success) {
    Write-Error "Impossible de demarrer la demande d'autorisation."
    exit 1
}
$code = $demande.code
$urlAuth = "$base/autoriser-appareil?code=$code"

Write-Host ""
Write-Host "Ouvre cette page et clique Autoriser (tentative d'ouverture automatique) :"
Write-Host $urlAuth -ForegroundColor Yellow
Write-Host ""
try { Start-Process $urlAuth } catch {}

Write-Host "En attente de ton autorisation..." -NoNewline
$jeton = $null
for ($i = 0; $i -lt 150; $i++) {
    Start-Sleep -Seconds 2
    Write-Host "." -NoNewline
    $statut = Invoke-RestMethod -Uri "$base/api/appareil/statut?code=$code"
    if ($statut.statut -eq 'approuve') { $jeton = $statut.jeton; break }
    if ($statut.statut -eq 'refuse') { Write-Host ""; Write-Error "Autorisation refusee."; exit 1 }
    if ($statut.statut -eq 'expire' -or $statut.statut -eq 'introuvable') {
        Write-Host ""
        Write-Error "Code expire -- relance le script."
        exit 1
    }
}
Write-Host ""
if (-not $jeton) {
    Write-Error "Delai depasse (5 min) -- relance le script."
    exit 1
}

$dossier = "$env:LOCALAPPDATA\VexCloudSync"
New-Item -ItemType Directory -Force -Path $dossier | Out-Null
$exePath = Join-Path $dossier "vex-cloudsync.exe"

Write-Host "Telechargement de vex-cloudsync.exe..."
Invoke-WebRequest -Uri "$base/api/appareil/telecharger" -Headers @{ Authorization = "Bearer $jeton" } -OutFile $exePath

Write-Host "Installe : $exePath" -ForegroundColor Green
Write-Host "Windows peut afficher un avertissement SmartScreen au premier lancement -- c'est normal pour un logiciel encore peu diffuse, clique 'Informations complementaires' puis 'Executer quand meme'." -ForegroundColor DarkGray
Start-Process $exePath
"#;

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
