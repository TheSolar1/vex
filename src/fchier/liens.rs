// ══════════════════════════════════════════════════════════════════
// fchier/liens.rs — Partage de fichier par lien public (ExoDrive)
//
// Chiffrement de bout en bout conserve (meme principe que Mega) :
//   1. le navigateur du proprietaire dechiffre le fichier avec sa cle
//      personnelle, le rechiffre avec une cle AES-256-GCM ALEATOIRE, et
//      envoie ce blob rechiffre (IV || ciphertext) au serveur ;
//   2. la cle aleatoire est placee dans le FRAGMENT du lien
//      (/partage/<jeton>#<cle>) : un navigateur n'envoie jamais le
//      fragment au serveur -- VEX ne peut pas lire le fichier partage ;
//   3. le visiteur telecharge le blob et le dechiffre localement.
// Le serveur controle seulement l'acces : mot de passe optionnel
// (bcrypt), date d'expiration, nombre maximal de telechargements,
// limitation des essais de mot de passe.
// ══════════════════════════════════════════════════════════════════

use super::fchier::{
    ecrire_sur_disque, html_response, json_response, lire_contenu_b64, parse_json_body,
    stockage_disque_config,
};
use crate::appeldb::{inserer_avec_erreur, inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tiny_http::{Request, Response};

type Resp = Response<std::io::Cursor<Vec<u8>>>;

const PREFIXE_DISQUE: &str = "DISK:";
/// Surcout max du rechiffrement (IV + tag GCM) par rapport a la taille
/// du fichier d'origine, avec une marge.
const SURCOUT_MAX: i64 = 64 * 1024;
const TAILLE_MAX_LIEN: i64 = 512 * 1024 * 1024;
const LIENS_MAX_PAR_UTILISATEUR: usize = 200;
const ESSAIS_MDP_MAX: u32 = 10;
const FENETRE_ESSAIS: Duration = Duration::from_secs(600);

fn nouveau_jeton() -> String {
    let mut b = [0u8; 24];
    getrandom::getrandom(&mut b).expect("getrandom");
    hex::encode(b)
}

fn jeton_valide(j: &str) -> bool {
    j.len() == 48 && j.bytes().all(|c| c.is_ascii_hexdigit())
}

fn maintenant() -> chrono::NaiveDateTime {
    chrono::Local::now().naive_local()
}

fn parse_date(s: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()
}

// ── Limitation des essais de mot de passe (par jeton) ─────────────
fn essais() -> &'static Mutex<HashMap<String, (u32, Instant)>> {
    static E: std::sync::OnceLock<Mutex<HashMap<String, (u32, Instant)>>> = std::sync::OnceLock::new();
    E.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trop_d_essais(jeton: &str) -> bool {
    let mut m = essais().lock().unwrap_or_else(|e| e.into_inner());
    m.retain(|_, (_, t)| t.elapsed() < FENETRE_ESSAIS);
    m.get(jeton).map(|(n, _)| *n >= ESSAIS_MDP_MAX).unwrap_or(false)
}

fn noter_echec(jeton: &str) {
    let mut m = essais().lock().unwrap_or_else(|e| e.into_inner());
    let e = m.entry(jeton.to_string()).or_insert((0, Instant::now()));
    e.0 += 1;
}

// ══════════════════════════════════════════════════════════════════
// API proprietaire (session requise) : /api/fchier/lien_*
// ══════════════════════════════════════════════════════════════════

/// POST /api/fchier/lien_creer
/// { id_fichier, contenu (base64 IV||ciphertext), mot_de_passe?, expire_jours?, max_telechargements? }
pub fn api_creer(pool: &DbPool, req: &mut Request, uid: i64) -> Resp {
    let body = match parse_json_body(req) {
        Some(b) => b,
        None => return json_response(400, json!({"success":false,"error":"Corps invalide"})),
    };
    let id_fichier = body.get("id_fichier").and_then(|v| v.as_i64()).unwrap_or(0);
    let contenu = body.get("contenu").and_then(|v| v.as_str()).unwrap_or("");
    let mdp = body.get("mot_de_passe").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let expire_jours = body.get("expire_jours").and_then(|v| v.as_i64()).unwrap_or(0);
    let max_dl = body.get("max_telechargements").and_then(|v| v.as_i64()).unwrap_or(0);

    let fichier = match selectionner(
        pool,
        "fichiers",
        &[
            ("id", mysql::Value::from(id_fichier)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
        &["id", "nom", "type_fichier", "taille"],
        None,
        Some(1),
    )
    .into_iter()
    .next()
    {
        Some(f) => f,
        None => return json_response(403, json!({"success":false,"error":"Fichier introuvable"})),
    };
    let nb_liens = selectionner(
        pool,
        "fchier_liens",
        &[("id_utilisateur", mysql::Value::from(uid))],
        &["jeton"],
        None,
        None,
    )
    .len();
    if nb_liens >= LIENS_MAX_PAR_UTILISATEUR {
        return json_response(429, json!({"success":false,"error":"Trop de liens actifs — supprimez-en d'abord."}));
    }

    use base64::Engine as _;
    let octets = match base64::engine::general_purpose::STANDARD.decode(contenu) {
        Ok(b) if b.len() >= 12 + 16 => b,
        _ => return json_response(400, json!({"success":false,"error":"Contenu chiffré invalide"})),
    };
    let taille = fichier.get("taille").and_then(|v| v.as_i64()).unwrap_or(0);
    if octets.len() as i64 > TAILLE_MAX_LIEN || (taille > 0 && octets.len() as i64 > taille + SURCOUT_MAX) {
        return json_response(413, json!({"success":false,"error":"Contenu trop volumineux"}));
    }
    if mdp.len() > 200 {
        return json_response(400, json!({"success":false,"error":"Mot de passe trop long"}));
    }
    if !(0..=365).contains(&expire_jours) || !(0..=100_000).contains(&max_dl) {
        return json_response(400, json!({"success":false,"error":"Paramètres invalides"}));
    }

    let valeur = stockage_disque_config()
        .and_then(|d| ecrire_sur_disque(&d, &octets))
        .unwrap_or_else(|| contenu.to_string());
    let mdp_hash = if mdp.is_empty() {
        mysql::Value::NULL
    } else {
        match bcrypt::hash(&mdp, 10) {
            Ok(h) => mysql::Value::from(h),
            Err(_) => return json_response(500, json!({"success":false,"error":"Hachage impossible"})),
        }
    };
    let expire = if expire_jours > 0 {
        mysql::Value::from(
            (maintenant() + chrono::Duration::days(expire_jours))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
    } else {
        mysql::Value::NULL
    };
    let jeton = nouveau_jeton();
    let res = inserer_avec_erreur(
        pool,
        "fchier_liens",
        &[
            ("jeton", mysql::Value::from(jeton.as_str())),
            ("id_utilisateur", mysql::Value::from(uid)),
            ("id_fichier", mysql::Value::from(id_fichier)),
            ("nom", fichier.get("nom").and_then(|v| v.as_str()).map(mysql::Value::from).unwrap_or(mysql::Value::from(""))),
            ("mime", fichier.get("type_fichier").and_then(|v| v.as_str()).map(mysql::Value::from).unwrap_or(mysql::Value::from(""))),
            ("taille", mysql::Value::from(taille)),
            ("contenu", mysql::Value::from(valeur.as_str())),
            ("mdp_hash", mdp_hash),
            ("expire_le", expire),
            ("max_telechargements", if max_dl > 0 { mysql::Value::from(max_dl) } else { mysql::Value::NULL }),
        ],
    );
    match res {
        Ok(_) => json_response(200, json!({"success":true,"jeton":jeton,"url":format!("/partage/{}", jeton)})),
        Err(e) => {
            if let Some(ch) = valeur.strip_prefix(PREFIXE_DISQUE) {
                let _ = std::fs::remove_file(ch);
            }
            json_response(500, json!({"success":false,"error":format!("Création impossible : {}", e)}))
        }
    }
}

/// GET /api/fchier/liens[?id_fichier=N] -- liens de l'utilisateur.
pub fn api_lister(pool: &DbPool, req: &Request, uid: i64) -> Resp {
    let url = req.url().to_string();
    let id_fichier = crate::utils::parse_query(&url)
        .get("id_fichier")
        .and_then(|v| v.parse::<i64>().ok());
    let mut filtre = vec![("id_utilisateur", mysql::Value::from(uid))];
    if let Some(f) = id_fichier {
        filtre.push(("id_fichier", mysql::Value::from(f)));
    }
    let rows = selectionner(
        pool,
        "fchier_liens",
        &filtre,
        &["jeton", "id_fichier", "nom", "taille", "mdp_hash", "expire_le", "telechargements", "max_telechargements", "cree_le"],
        Some("cree_le DESC"),
        Some(500),
    );
    let now = maintenant();
    let liens: Vec<Value> = rows
        .iter()
        .map(|r| {
            let expire = r.get("expire_le").and_then(|v| v.as_str()).and_then(parse_date);
            let dl = r.get("telechargements").and_then(|v| v.as_i64()).unwrap_or(0);
            let max = r.get("max_telechargements").and_then(|v| v.as_i64());
            json!({
                "jeton": r.get("jeton").cloned().unwrap_or(json!("")),
                "id_fichier": r.get("id_fichier").cloned().unwrap_or(json!(0)),
                "nom": r.get("nom").cloned().unwrap_or(json!("")),
                "taille": r.get("taille").cloned().unwrap_or(json!(0)),
                "mot_de_passe": r.get("mdp_hash").map(|v| !v.is_null()).unwrap_or(false),
                "expire_le": r.get("expire_le").cloned().unwrap_or(Value::Null),
                "expire": expire.map(|e| e < now).unwrap_or(false) || max.map(|m| dl >= m).unwrap_or(false),
                "telechargements": dl,
                "max_telechargements": max,
                "cree_le": r.get("cree_le").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    json_response(200, json!({"success":true,"liens":liens}))
}

fn supprimer_lien(pool: &DbPool, ligne: &HashMap<String, Value>) {
    if let Some(ch) = ligne
        .get("contenu")
        .and_then(|v| v.as_str())
        .and_then(|s| s.strip_prefix(PREFIXE_DISQUE))
    {
        let _ = std::fs::remove_file(ch);
    }
    if let Some(j) = ligne.get("jeton").and_then(|v| v.as_str()) {
        supprimer_ligne(pool, "fchier_liens", "jeton", mysql::Value::from(j));
    }
}

/// POST /api/fchier/lien_supprimer { jeton }
pub fn api_supprimer(pool: &DbPool, req: &mut Request, uid: i64) -> Resp {
    let jeton = parse_json_body(req)
        .and_then(|b| b.get("jeton").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_default();
    if !jeton_valide(&jeton) {
        return json_response(400, json!({"success":false,"error":"Jeton invalide"}));
    }
    match selectionner(
        pool,
        "fchier_liens",
        &[
            ("jeton", mysql::Value::from(jeton.as_str())),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
        &["jeton", "contenu"],
        None,
        Some(1),
    )
    .into_iter()
    .next()
    {
        Some(l) => {
            supprimer_lien(pool, &l);
            json_response(200, json!({"success":true}))
        }
        None => json_response(404, json!({"success":false,"error":"Lien introuvable"})),
    }
}

// ══════════════════════════════════════════════════════════════════
// Acces public : /partage/<jeton> (page) et /api/partage/<jeton> (POST)
// ══════════════════════════════════════════════════════════════════

enum Etat {
    Introuvable,
    Expire,
    Ok(HashMap<String, Value>),
}

fn charger_public(pool: &DbPool, jeton: &str) -> Etat {
    if !jeton_valide(jeton) {
        return Etat::Introuvable;
    }
    let l = match selectionner(
        pool,
        "fchier_liens",
        &[("jeton", mysql::Value::from(jeton))],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()
    {
        Some(l) => l,
        None => return Etat::Introuvable,
    };
    let expire = l.get("expire_le").and_then(|v| v.as_str()).and_then(parse_date);
    let dl = l.get("telechargements").and_then(|v| v.as_i64()).unwrap_or(0);
    let max = l.get("max_telechargements").and_then(|v| v.as_i64());
    if expire.map(|e| e < maintenant()).unwrap_or(false) || max.map(|m| dl >= m).unwrap_or(false) {
        return Etat::Expire;
    }
    Etat::Ok(l)
}

pub fn handle_public(pool: &DbPool, req: &mut Request) -> Resp {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    if let Some(jeton) = path.strip_prefix("/api/partage/") {
        let jeton = jeton.trim_end_matches('/').to_string();
        return api_public(pool, req, &jeton);
    }
    html_response(PAGE_PUBLIQUE.to_string())
}

/// POST /api/partage/<jeton> { mot_de_passe?, info? }
///   info=true : metadonnees seules (le nom n'est revele qu'apres le
///   mot de passe s'il y en a un) ; sinon contenu chiffre + compteur.
fn api_public(pool: &DbPool, req: &mut Request, jeton: &str) -> Resp {
    if req.method().as_str() != "POST" {
        return json_response(405, json!({"success":false,"error":"POST requis"}));
    }
    let body = parse_json_body(req).unwrap_or(json!({}));
    let info = body.get("info").and_then(|v| v.as_bool()).unwrap_or(false);
    let mdp = body.get("mot_de_passe").and_then(|v| v.as_str()).unwrap_or("");

    let l = match charger_public(pool, jeton) {
        Etat::Introuvable => return json_response(404, json!({"success":false,"error":"Ce lien n'existe pas ou a été supprimé."})),
        Etat::Expire => return json_response(410, json!({"success":false,"error":"Ce lien a expiré."})),
        Etat::Ok(l) => l,
    };
    let hash = l.get("mdp_hash").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let protege = !hash.is_empty();
    if info && (!protege || mdp.is_empty()) {
        return json_response(200, json!({
            "success": true,
            "mot_de_passe": protege,
            "nom": if protege { Value::Null } else { l.get("nom").cloned().unwrap_or(json!("")) },
            "taille": if protege { Value::Null } else { l.get("taille").cloned().unwrap_or(json!(0)) },
            "expire_le": l.get("expire_le").cloned().unwrap_or(Value::Null),
        }));
    }
    if protege {
        if trop_d_essais(jeton) {
            return json_response(429, json!({"success":false,"error":"Trop d'essais — réessayez dans 10 minutes."}));
        }
        if !bcrypt::verify(mdp, &hash).unwrap_or(false) {
            noter_echec(jeton);
            return json_response(403, json!({"success":false,"error":"Mot de passe incorrect."}));
        }
    }
    if info {
        return json_response(200, json!({
            "success": true,
            "mot_de_passe": protege,
            "nom": l.get("nom").cloned().unwrap_or(json!("")),
            "taille": l.get("taille").cloned().unwrap_or(json!(0)),
            "expire_le": l.get("expire_le").cloned().unwrap_or(Value::Null),
        }));
    }
    let contenu = match l.get("contenu").and_then(|v| v.as_str()).map(lire_contenu_b64) {
        Some(Ok(c)) => c,
        _ => return json_response(500, json!({"success":false,"error":"Contenu illisible"})),
    };
    let dl = l.get("telechargements").and_then(|v| v.as_i64()).unwrap_or(0);
    inserer_ou_modifier(
        pool,
        "fchier_liens",
        &[("telechargements", mysql::Value::from(dl + 1))],
        &[("jeton", mysql::Value::from(jeton))],
    );
    json_response(200, json!({
        "success": true,
        "nom": l.get("nom").cloned().unwrap_or(json!("")),
        "mime": l.get("mime").cloned().unwrap_or(json!("application/octet-stream")),
        "contenu": contenu,
    }))
}

/// Supprime les liens d'un fichier (appele quand le fichier est purge).
pub fn supprimer_liens_fichier(pool: &DbPool, uid: i64, id_fichier: i64) {
    for l in selectionner(
        pool,
        "fchier_liens",
        &[
            ("id_fichier", mysql::Value::from(id_fichier)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
        &["jeton", "contenu"],
        None,
        None,
    ) {
        supprimer_lien(pool, &l);
    }
}

const PAGE_PUBLIQUE: &str = r#"<!DOCTYPE html>
<html lang="fr" data-theme="light">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="robots" content="noindex">
<title>Fichier partagé — VEX</title>
<link rel="stylesheet" href="/static/css/theme.css">
<style>
  body{margin:0;padding:0!important;font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center}
  .carte{width:min(420px,calc(100% - 32px));padding:28px 24px;text-align:center}
  h1{font-size:1.15rem;margin:10px 0 4px;word-break:break-word}
  .dim{color:var(--text-dim);font-size:.85rem}
  input{width:100%;box-sizing:border-box;padding:11px 12px;border-radius:10px;margin:14px 0 10px;font-size:1rem}
  button{width:100%;padding:12px;border:0;border-radius:10px;font-weight:700;font-size:1rem;cursor:pointer;background:var(--accent);color:#fff}
  button:disabled{opacity:.6;cursor:default}
  .err{color:var(--danger);margin-top:12px;font-size:.9rem;min-height:1.2em}
  .logo{width:56px;height:56px;border-radius:14px;background:linear-gradient(135deg,var(--vex-green-1),var(--vex-green-2));display:inline-flex;align-items:center;justify-content:center}
  .logo img{width:28px;height:28px;filter:brightness(0) invert(1)}
  @media (prefers-color-scheme: dark){:root{color-scheme:dark}}
</style>
</head>
<body>
<main class="carte vex-card" aria-live="polite">
  <div class="logo"><img src="/static/img/solid/file-arrow-down.svg" alt=""></div>
  <h1 id="nom">Fichier partagé</h1>
  <div class="dim" id="meta">Chargement…</div>
  <form id="form" hidden>
    <label for="mdp" class="dim" id="mdp-label" hidden>Ce fichier est protégé par un mot de passe.</label>
    <input id="mdp" class="vex-input" type="password" autocomplete="off" placeholder="Mot de passe" hidden>
    <button id="btn" type="submit">Télécharger</button>
  </form>
  <div class="err" id="err" role="alert"></div>
  <p class="dim" style="margin-top:18px;font-size:.75rem">Chiffré de bout en bout : la clé est dans le lien, le serveur ne peut pas lire ce fichier.</p>
</main>
<script>
(function(){
  if (window.matchMedia && matchMedia('(prefers-color-scheme: dark)').matches) document.documentElement.setAttribute('data-theme','dark');
  const jeton = location.pathname.split('/').filter(Boolean)[1] || '';
  const cleB64 = location.hash.slice(1);
  const $ = id => document.getElementById(id);
  const api = '/api/partage/' + encodeURIComponent(jeton);
  function taille(o){ if(!o&&o!==0) return ''; if(o<1024) return o+' o'; if(o<1048576) return (o/1024).toFixed(1)+' Ko'; if(o<1073741824) return (o/1048576).toFixed(1)+' Mo'; return (o/1073741824).toFixed(2)+' Go'; }
  function b64(s){ s=s.replace(/-/g,'+').replace(/_/g,'/'); while(s.length%4) s+='='; const b=atob(s); const u=new Uint8Array(b.length); for(let i=0;i<b.length;i++) u[i]=b.charCodeAt(i); return u; }
  function erreur(m){ $('err').textContent = m; }
  async function post(corps){ const r = await fetch(api,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(corps)}); return r.json(); }
  let protege = false;
  function afficher(d){
    if (d.nom) { $('nom').textContent = d.nom; document.title = d.nom + ' — VEX'; }
    const parts = [];
    if (d.taille != null) parts.push(taille(d.taille));
    if (d.expire_le) parts.push('expire le ' + String(d.expire_le).slice(0,16).replace('T',' '));
    $('meta').textContent = parts.join(' · ');
  }
  async function init(){
    if (!cleB64) { $('meta').textContent=''; erreur('Lien incomplet : la clé de déchiffrement (après le #) est manquante.'); return; }
    try {
      const d = await post({info:true});
      if (!d.success) { $('meta').textContent=''; erreur(d.error||'Lien invalide.'); return; }
      protege = !!d.mot_de_passe;
      afficher(d);
      $('form').hidden = false;
      $('mdp').hidden = $('mdp-label').hidden = !protege;
      if (protege) $('mdp').focus();
    } catch(e){ erreur('Serveur injoignable.'); }
  }
  $('form').addEventListener('submit', async e => {
    e.preventDefault(); erreur(''); $('btn').disabled = true; $('btn').textContent = 'Téléchargement…';
    try {
      const d = await post({mot_de_passe: $('mdp').value});
      if (!d.success) throw new Error(d.error || 'Erreur');
      afficher(d);
      $('btn').textContent = 'Déchiffrement…';
      const brut = b64(d.contenu);
      const cle = await crypto.subtle.importKey('raw', b64(cleB64), 'AES-GCM', false, ['decrypt']);
      const clair = await crypto.subtle.decrypt({name:'AES-GCM', iv: brut.slice(0,12)}, cle, brut.slice(12));
      const url = URL.createObjectURL(new Blob([clair], {type: d.mime || 'application/octet-stream'}));
      const a = document.createElement('a'); a.href = url; a.download = d.nom || 'fichier';
      document.body.appendChild(a); a.click(); setTimeout(()=>{ a.remove(); URL.revokeObjectURL(url); }, 1500);
      $('btn').textContent = 'Télécharger à nouveau';
    } catch(err) {
      erreur(err && err.name === 'OperationError' ? 'Clé de déchiffrement invalide (lien tronqué ?).' : (err.message || 'Erreur'));
      $('btn').textContent = 'Télécharger';
    } finally { $('btn').disabled = false; }
  });
  init();
})();
</script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jetons() {
        let j = nouveau_jeton();
        assert!(jeton_valide(&j));
        assert_ne!(j, nouveau_jeton());
        assert!(!jeton_valide("../../etc"));
        assert!(!jeton_valide(&"g".repeat(48)));
    }

    #[test]
    fn limite_essais() {
        let j = nouveau_jeton();
        for _ in 0..ESSAIS_MDP_MAX {
            assert!(!trop_d_essais(&j));
            noter_echec(&j);
        }
        assert!(trop_d_essais(&j));
    }
}
