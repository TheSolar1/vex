// ══════════════════════════════════════════════════════════════════
// sso.rs — « Se connecter avec VEX » pour les services externes
// (ex. le jeu WorldFront). Fichier autonome : seul main.rs y renvoie
// les routes /p2p/sso et /p2p/sso/etat.
//
//   1. le service redirige vers  GET /p2p/sso?service=..&retour=..&etat=..
//   2. ce nœud verifie la session VEX et demande le consentement
//   3. « Autoriser » (POST) : le nœud signe l'identite avec sa cle P2P
//      Ed25519 et renvoie vers  retour?etat=..&jeton=..&sig=..
//   4. le service verifie la signature (cle publique : /p2p/ping)
//
// Jeton = base64url(JSON) { v, node_id, user_id, nom, sombre, retour,
// etat, iat, exp }, signe sur sa chaine base64url. Ni email, ni mot de
// passe, ni fichier ne sont transmis.
//
// Le cookie VEX est en SameSite=Strict : il n'est pas envoye quand on
// arrive depuis un autre site. La premiere reponse est donc une page qui
// se recharge (navigation lancee par ce site -> cookie envoye).
// Pas connecte : la connexion VEX s'ouvre dans un onglet, et cette page
// continue seule des que la session existe (/p2p/sso/etat) -- le login
// de VEX n'est pas modifie.
//
// « Ne plus me demander » : les domaines deja autorises sont retenus
// dans le cookie `vex_sso_ok` (HttpOnly, Path=/p2p/sso), signe par la
// cle du nœud et lie au compte : un autre compte sur le meme navigateur
// revoit la page d'accord. Effacer ce cookie retire toutes les autorisations.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::function::html_escape;
use crate::p2p::p2p::NodeState;
use crate::utils::{parse_query, url_decode};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use ed25519_dalek::Signer;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tiny_http::{Header, Request, Response};

/// Duree de validite d'un jeton signe (secondes).
const VALIDITE_JETON: i64 = 300;
/// Nom et duree (secondes) du cookie des domaines autorises.
const COOKIE_ACCORDS: &str = "vex_sso_ok";
const DUREE_ACCORDS: i64 = 180 * 24 * 3600;
/// Nombre maximal de domaines retenus.
const MAX_ACCORDS: usize = 20;

pub fn traiter(mut request: Request, pool: &DbPool, node_state: &Arc<RwLock<NodeState>>) {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url).to_string();
    let method = request.method().to_string();
    let session = crate::access_control::check_connected(pool, &request);

    // Petit test utilise par la page d'attente (meme site : cookie envoye).
    if path == "/p2p/sso/etat" {
        let _ = request.respond(
            Response::from_string(json!({ "connecte": session.is_some() }).to_string())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap()),
        );
        return;
    }

    let query = parse_query(&url);
    let body = if method == "POST" { lire_formulaire(&mut request) } else { HashMap::new() };
    let params = if method == "POST" { &body } else { &query };
    let service: String = params.get("service").cloned().unwrap_or_default().chars().take(40).collect();
    let retour = params.get("retour").cloned().unwrap_or_default();
    let etat = params.get("etat").cloned().unwrap_or_default();

    if !retour_valide(&retour) || etat.is_empty() || etat.len() > 128 || !etat.chars().all(|c| c.is_ascii_alphanumeric()) {
        return page(request, "Demande invalide", "<p>Le lien de connexion est incomplet ou mal formé.</p>", "light");
    }
    let service = if service.trim().is_empty() { "Service externe".to_string() } else { service };
    let ici = format!("/p2p/sso?service={}&retour={}&etat={}&r=1", enc(&service), enc(&retour), enc(&etat));

    if method == "GET" {
        // Arrivee depuis le service : recharge une fois pour avoir le cookie.
        if query.get("r").map(|s| s.as_str()) != Some("1") {
            let html = format!(
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"0;url={u}\">\
                 <script>location.replace({j});</script></head><body></body></html>",
                u = html_escape(&ici),
                j = serde_json::to_string(&ici).unwrap_or_default()
            );
            let _ = request.respond(
                Response::from_string(html)
                    .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
                    .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap()),
            );
            return;
        }
        let Some(s) = session else {
            // Pas connecte : connexion VEX dans un onglet, puis on continue seul.
            let corps = format!(
                "<p class=\"sso-demande\"><b>{service}</b> veut vérifier votre identité VEX.</p>\
                 <div class=\"sso-cadre\">Connectez-vous d'abord à VEX : cette page continuera toute seule.</div>\
                 <div class=\"sso-boutons\"><a class=\"sso-btn\" href=\"/login\" target=\"_blank\" rel=\"opener\">Se connecter à VEX</a></div>\
                 <p class=\"sso-petit\" id=\"attente\">En attente de votre connexion…</p>\
                 <script>setInterval(function(){{fetch('/p2p/sso/etat',{{cache:'no-store'}}).then(function(r){{return r.json()}})\
                 .then(function(d){{if(d.connecte)location.reload()}}).catch(function(){{}})}},2000);</script>",
                service = html_escape(&service)
            );
            return page(request, "Connexion à un service externe", &corps, "light");
        };
        // Domaine deja autorise par ce compte : pas de page d'accord.
        if accords(&request, node_state, s.user_id).iter().any(|d| *d == domaine(&retour)) {
            let dest = emettre(pool, node_state, &s, &retour, &etat);
            return rediriger(request, &dest, None);
        }
        let refus = format!("{}{}erreur=refus&etat={}", retour, sep(&retour), etat);
        let corps = format!(
            "<p class=\"sso-demande\"><b>{service}</b> demande à vérifier votre identité VEX.</p>\
             <div class=\"sso-cadre\">\
               <div>Connecté sur ce nœud en tant que <b>{nom}</b></div>\
               <div>Vous serez renvoyé vers <b>{domaine}</b></div>\
             </div>\
             <p class=\"sso-petit\">Le service recevra uniquement votre nom, votre identifiant sur ce nœud et votre thème. \
             Jamais votre mot de passe, votre email ni vos fichiers.</p>\
             <form method=\"post\" action=\"/p2p/sso\">\
               <input type=\"hidden\" name=\"service\" value=\"{hs}\">\
               <input type=\"hidden\" name=\"retour\" value=\"{hr}\">\
               <input type=\"hidden\" name=\"etat\" value=\"{he}\">\
               <label class=\"sso-petit sso-retenir\"><input type=\"checkbox\" name=\"retenir\" value=\"1\" checked> \
                 Ne plus me demander pour <b>{domaine}</b> sur ce navigateur</label>\
               <div class=\"sso-boutons\">\
                 <a class=\"sso-btn sec\" href=\"{hrefus}\">Refuser</a>\
                 <button class=\"sso-btn\" type=\"submit\" name=\"decision\" value=\"ok\">Autoriser</button>\
               </div>\
             </form>",
            service = html_escape(&service),
            nom = html_escape(&s.user_nom),
            domaine = html_escape(&domaine(&retour)),
            hs = html_escape(&service),
            hr = html_escape(&retour),
            he = html_escape(&etat),
            hrefus = html_escape(&refus),
        );
        return page(request, "Connexion à un service externe", &corps, theme_de(pool, s.user_id));
    }

    // ── POST : consentement (protege du CSRF par le cookie Strict) ──
    let Some(s) = session else {
        return page(request, "Session expirée", "<p>Votre session VEX a expiré. Recommencez depuis le service.</p>", "light");
    };
    if body.get("decision").map(|d| d.as_str()) != Some("ok") {
        return rediriger(request, &format!("{}{}erreur=refus&etat={}", retour, sep(&retour), etat), None);
    }
    let cookie = if body.get("retenir").map(|d| d.as_str()) == Some("1") {
        let mut liste = accords(&request, node_state, s.user_id);
        let d = domaine(&retour);
        liste.retain(|x| *x != d);
        liste.insert(0, d);
        liste.truncate(MAX_ACCORDS);
        Some(cookie_accords(node_state, s.user_id, &liste))
    } else {
        None
    };
    let dest = emettre(pool, node_state, &s, &retour, &etat);
    rediriger(request, &dest, cookie.as_deref());
}

/// Signe l'identite de l'utilisateur et renvoie l'URL de retour du service.
fn emettre(pool: &DbPool, node_state: &Arc<RwLock<NodeState>>, s: &crate::c::SessionInfo, retour: &str, etat: &str) -> String {
    let ns = node_state.read().unwrap();
    let maintenant = chrono::Utc::now().timestamp();
    let charge = json!({
        "v": 1,
        "node_id": ns.node_id,
        "user_id": s.user_id,
        "nom": s.user_nom,
        "sombre": theme_de(pool, s.user_id) == "dark",
        "retour": retour,
        "etat": etat,
        "iat": maintenant,
        "exp": maintenant + VALIDITE_JETON,
    });
    let jeton = B64URL.encode(charge.to_string().as_bytes());
    let sig = B64URL.encode(ns.signing_key.sign(jeton.as_bytes()).to_bytes());
    drop(ns);
    format!("{}{}etat={}&jeton={}&sig={}", retour, sep(retour), etat, jeton, sig)
}

/// Domaines deja autorises par `user_id` (cookie absent, altere ou d'un
/// autre compte : liste vide).
fn accords(request: &Request, node_state: &Arc<RwLock<NodeState>>, user_id: i64) -> Vec<String> {
    use ed25519_dalek::{Signature, Verifier};
    let brut = crate::access_control::get_cookie(request, COOKIE_ACCORDS);
    let Some((charge, sig)) = brut.split_once('.') else { return vec![] };
    let Ok(sig) = B64URL.decode(sig) else { return vec![] };
    let Ok(sig) = Signature::from_slice(&sig) else { return vec![] };
    if node_state.read().unwrap().verifying_key.verify(charge.as_bytes(), &sig).is_err() {
        return vec![];
    }
    let Ok(octets) = B64URL.decode(charge) else { return vec![] };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&octets) else { return vec![] };
    if v["u"].as_i64() != Some(user_id) {
        return vec![];
    }
    v["d"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn cookie_accords(node_state: &Arc<RwLock<NodeState>>, user_id: i64, domaines: &[String]) -> String {
    let charge = B64URL.encode(json!({ "u": user_id, "d": domaines }).to_string().as_bytes());
    let sig = B64URL.encode(node_state.read().unwrap().signing_key.sign(charge.as_bytes()).to_bytes());
    format!("{}={}.{}; Path=/p2p/sso; Max-Age={}; HttpOnly; SameSite=Lax", COOKIE_ACCORDS, charge, sig, DUREE_ACCORDS)
}

fn lire_formulaire(request: &mut Request) -> HashMap<String, String> {
    use std::io::Read;
    let mut s = String::new();
    let _ = request.as_reader().take(16 * 1024).read_to_string(&mut s);
    s.split('&')
        .filter_map(|p| {
            let mut kv = p.splitn(2, '=');
            Some((url_decode(kv.next()?), url_decode(kv.next().unwrap_or(""))))
        })
        .collect()
}

fn retour_valide(r: &str) -> bool {
    (r.starts_with("https://") || r.starts_with("http://"))
        && r.len() < 500
        && !r.chars().any(|c| c.is_whitespace() || c == '"' || c == '<' || c == '>' || c == '\'' || c == '#')
}

fn domaine(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("").to_string()
}

fn sep(url: &str) -> &'static str {
    if url.contains('?') { "&" } else { "?" }
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

fn theme_de(pool: &DbPool, user_id: i64) -> &'static str {
    if crate::function::get_user_preferences(pool, user_id).teme == 1 { "dark" } else { "light" }
}

fn rediriger(request: Request, location: &str, cookie: Option<&str>) {
    let mut r = Response::empty(302)
        .with_header(Header::from_bytes("Location", location).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
    if let Some(c) = cookie {
        r = r.with_header(Header::from_bytes("Set-Cookie", c).unwrap());
    }
    let _ = request.respond(r);
}

fn page(request: Request, titre: &str, corps: &str, theme: &str) {
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="fr" data-theme="{theme}">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{titre} — VEX</title>
<link rel="icon" href="/static/img/favicon.ico">
<link rel="stylesheet" href="/static/css/theme.css?v=6">
<style>
  body {{ margin:0; min-height:100vh; display:flex; align-items:center; justify-content:center; padding:20px; box-sizing:border-box;
         font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif; background:var(--bg); color:var(--text); }}
  .sso {{ width:460px; max-width:100%; background:var(--surface); border:1px solid var(--panel-border); border-radius:var(--radius);
          box-shadow:var(--shadow); padding:28px; }}
  .sso-tete {{ display:flex; align-items:center; gap:14px; padding-bottom:16px; margin-bottom:16px; border-bottom:2px solid var(--panel-border); }}
  .sso-tete img {{ width:42px; height:42px; filter:grayscale(100%) brightness(.65); }}
  .sso-tete h1 {{ font-size:19px; margin:0; }}
  .sso-demande {{ font-size:15px; line-height:1.5; }}
  .sso-cadre {{ background:var(--alt); border-radius:10px; padding:12px 14px; display:flex; flex-direction:column; gap:6px; font-size:14px; }}
  .sso-petit {{ font-size:12.5px; color:var(--text-dim); line-height:1.5; }}
  .sso-retenir {{ display:flex; align-items:center; gap:7px; margin-top:12px; cursor:pointer; }}
  .sso-boutons {{ display:flex; justify-content:flex-end; gap:8px; margin-top:18px; }}
  .sso-btn {{ padding:9px 18px; border-radius:8px; border:1px solid transparent; background:var(--accent); color:#fff; font-weight:600;
              font-size:14px; cursor:pointer; text-decoration:none; }}
  .sso-btn.sec {{ background:var(--surface2); color:var(--text); border-color:var(--panel-border); }}
</style>
</head>
<body>
  <div class="sso">
    <div class="sso-tete"><img src="/static/img/vex.svg" alt="VEX"><h1>{titre}</h1></div>
    {corps}
  </div>
</body>
</html>"#,
        theme = theme,
        titre = html_escape(titre),
        corps = corps
    );
    let _ = request.respond(
        Response::from_string(html)
            .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("X-Frame-Options", "DENY").unwrap()),
    );
}
