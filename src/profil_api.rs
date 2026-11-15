// ══════════════════════════════════════════════════════════════════
// profil_api.rs — photo de profil + cles API personnelles.
//
// Demandes utilisateur : "on ne peut pas changer la PP" (les colonnes
// pref.profile_icon_type/url existaient mais rien ne les ecrivait) et
// "on ne peut pas avoir de cles API".
//
// PHOTO : le navigateur recadre/redimensionne l'image (256x256, JPEG) et
// l'envoie en data URL ; on verifie les octets magiques (PNG/JPEG/WEBP,
// jamais SVG) et on l'ecrit sous static/avatars/ (public, comme
// static/sitec_media) avec un nom aleatoire.
//
// CLES API : "vex_" + 40 hex (CSPRNG). Seul le hash SHA-256 est garde en
// base ; la cle n'est montree qu'une fois, a la creation. Envoi :
// "Authorization: Bearer vex_..." sur /api/fchier/* et /api/moi. Autorisees
// si le plan a "api_access" (Admin > Plans), ou pour le fondateur.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, DbPool};
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;

const DOSSIER_AVATARS: &str = "static/avatars";
const MAX_PHOTO: usize = 1024 * 1024;
const MAX_CLES: usize = 10;

fn aleatoire_hex(n_octets: usize) -> Option<String> {
    let mut b = vec![0u8; n_octets];
    getrandom::getrandom(&mut b).ok()?;
    Some(hex::encode(b))
}

// ── Photo de profil ────────────────────────────────────────────────

/// URL de la photo du compte, si elle en a une.
pub fn photo_url(pool: &DbPool, uid: i64) -> Option<String> {
    let p = crate::function::get_user_preferences(pool, uid);
    if p.profile_icon_type == "image" {
        p.profile_icon_url.filter(|u| u.starts_with("/static/avatars/"))
    } else {
        None
    }
}

fn ecrire_pref_photo(pool: &DbPool, uid: i64, type_: &str, url: Option<&str>) -> bool {
    // Cree la ligne pref si besoin (get_user_preferences l'insere).
    let _ = crate::function::get_user_preferences(pool, uid);
    inserer_ou_modifier(
        pool,
        "pref",
        &[
            ("profile_icon_type", mysql::Value::from(type_)),
            ("profile_icon_url", url.map(mysql::Value::from).unwrap_or(mysql::Value::NULL)),
        ],
        &[("id-user", mysql::Value::from(uid))],
    ) >= 0
}

fn supprimer_ancienne(pool: &DbPool, uid: i64) {
    if let Some(url) = photo_url(pool, uid) {
        let nom = url.trim_start_matches("/static/avatars/");
        if !nom.is_empty() && !nom.contains('/') && !nom.contains("..") {
            let _ = std::fs::remove_file(format!("{}/{}", DOSSIER_AVATARS, nom));
        }
    }
}

/// POST /api/account/photo : "image" = data URL (PNG/JPEG/WEBP), ou
/// "supprimer" = 1 pour revenir a l'initiale.
pub fn photo(pool: &DbPool, uid: i64, body: &HashMap<String, String>) -> Value {
    if body.get("supprimer").map(|v| v == "1").unwrap_or(false) {
        supprimer_ancienne(pool, uid);
        return if ecrire_pref_photo(pool, uid, "initials", None) {
            json!({"success": true, "photo": null})
        } else {
            json!({"success": false, "error": "Enregistrement impossible."})
        };
    }
    let donnees = body.get("image").map(|s| s.as_str()).unwrap_or("");
    let b64 = donnees.split_once(',').map(|(_, b)| b).unwrap_or(donnees);
    let octets = match base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
        Ok(o) => o,
        Err(_) => return json!({"success": false, "error": "Image illisible."}),
    };
    if octets.is_empty() || octets.len() > MAX_PHOTO {
        return json!({"success": false, "error": "Image trop lourde (1 Mo maximum)."});
    }
    let ext = if octets.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if octets.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if octets.len() > 12 && &octets[0..4] == b"RIFF" && &octets[8..12] == b"WEBP" {
        "webp"
    } else {
        return json!({"success": false, "error": "Format non accepté (PNG, JPEG ou WEBP)."});
    };
    let Some(alea) = aleatoire_hex(8) else {
        return json!({"success": false, "error": "Erreur interne."});
    };
    if std::fs::create_dir_all(DOSSIER_AVATARS).is_err() {
        return json!({"success": false, "error": "Dossier des photos inaccessible."});
    }
    let nom = format!("{}-{}.{}", uid, alea, ext);
    if std::fs::write(format!("{}/{}", DOSSIER_AVATARS, nom), &octets).is_err() {
        return json!({"success": false, "error": "Écriture impossible."});
    }
    supprimer_ancienne(pool, uid);
    let url = format!("/static/avatars/{}", nom);
    if ecrire_pref_photo(pool, uid, "image", Some(&url)) {
        json!({"success": true, "photo": url})
    } else {
        json!({"success": false, "error": "Enregistrement impossible."})
    }
}

// ── Cles API ───────────────────────────────────────────────────────

fn hasher(cle: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"vex-api:");
    h.update(cle.as_bytes());
    format!("{:x}", h.finalize())
}

/// Le compte a-t-il droit aux cles API ? Fondateur : toujours. Sinon le
/// plan doit avoir "api_access" (sauf si les restrictions de plans sont
/// coupees dans la config).
pub fn api_autorisee(pool: &DbPool, uid: i64, privilege: i64) -> bool {
    if privilege == 1 {
        return true;
    }
    let cfg = crate::config_loader::load_config("config.json");
    if !cfg.plans.enforce_plan_restrictions {
        return true;
    }
    let vip = selectionner(pool, "login", &[("id", mysql::Value::from(uid))], &["vip"], None, Some(1))
        .into_iter()
        .next()
        .map(|u| crate::utils::vip_brut_json(u.get("vip")))
        .unwrap_or_default();
    let plan_id = crate::utils::plan_depuis_vip(&vip);
    let plan = cfg
        .plans
        .available_plans
        .iter()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(plan_id.as_str()))
        .or_else(|| cfg.plans.available_plans.iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some("free")));
    plan.and_then(|p| p.pointer("/features/api_access")).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn lignes_cles(pool: &DbPool, uid: i64) -> Vec<HashMap<String, Value>> {
    selectionner(
        pool,
        "api_cles",
        &[("user_id", mysql::Value::from(uid)), ("revoque", mysql::Value::from(0i64))],
        &["id", "nom", "prefixe", "cree", "derniere_utilisation"],
        Some("id DESC"),
        None,
    )
}

/// GET /api/account/cles
pub fn lister(pool: &DbPool, uid: i64, privilege: i64) -> Value {
    let cles: Vec<Value> = lignes_cles(pool, uid)
        .into_iter()
        .map(|r| {
            json!({
                "id": r.get("id").cloned().unwrap_or(json!(0)),
                "nom": r.get("nom").cloned().unwrap_or(json!("")),
                "prefixe": r.get("prefixe").cloned().unwrap_or(json!("")),
                "cree": r.get("cree").cloned().unwrap_or(json!("")),
                "derniere_utilisation": r.get("derniere_utilisation").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    json!({"success": true, "autorise": api_autorisee(pool, uid, privilege), "cles": cles, "max": MAX_CLES})
}

/// POST /api/account/cles/creer (nom) -> la cle, montree UNE seule fois.
pub fn creer(pool: &DbPool, uid: i64, privilege: i64, nom: &str) -> Value {
    if !api_autorisee(pool, uid, privilege) {
        return json!({"success": false, "error": "Votre plan ne comprend pas l'accès API."});
    }
    if lignes_cles(pool, uid).len() >= MAX_CLES {
        return json!({"success": false, "error": format!("{} clés maximum : révoquez-en une d'abord.", MAX_CLES)});
    }
    let nom: String = nom.chars().filter(|c| !c.is_control()).take(60).collect();
    let nom = if nom.trim().is_empty() { "Clé API".to_string() } else { nom.trim().to_string() };
    let Some(alea) = aleatoire_hex(20) else {
        return json!({"success": false, "error": "Erreur interne."});
    };
    let cle = format!("vex_{}", alea);
    let prefixe: String = cle.chars().take(12).collect();
    let maintenant = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let id = inserer_ou_modifier(
        pool,
        "api_cles",
        &[
            ("user_id", mysql::Value::from(uid)),
            ("nom", mysql::Value::from(nom.as_str())),
            ("prefixe", mysql::Value::from(prefixe.as_str())),
            ("cle_hash", mysql::Value::from(hasher(&cle))),
            ("cree", mysql::Value::from(maintenant.as_str())),
            ("revoque", mysql::Value::from(0i64)),
        ],
        &[],
    );
    if id <= 0 {
        return json!({"success": false, "error": "Enregistrement impossible."});
    }
    json!({"success": true, "cle": cle, "id": id, "nom": nom})
}

/// POST /api/account/cles/revoquer (id)
pub fn revoquer(pool: &DbPool, uid: i64, id: i64) -> Value {
    let a_moi = !selectionner(
        pool,
        "api_cles",
        &[("id", mysql::Value::from(id)), ("user_id", mysql::Value::from(uid))],
        &["id"],
        None,
        Some(1),
    )
    .is_empty();
    if !a_moi {
        return json!({"success": false, "error": "Clé introuvable."});
    }
    inserer_ou_modifier(pool, "api_cles", &[("revoque", mysql::Value::from(1i64))], &[("id", mysql::Value::from(id))]);
    json!({"success": true})
}

/// Authentifie "Authorization: Bearer vex_..." -> meme forme que
/// appeldb::verifier_connexion (cles "connecte", "id", "nom", "email",
/// "privilege", "vip", "plan"). None si cle inconnue, revoquee, ou si le
/// plan du compte n'a plus l'acces API.
pub fn verifier(pool: &DbPool, cle: &str) -> Option<HashMap<String, Value>> {
    if !cle.starts_with("vex_") || cle.len() != 44 {
        return None;
    }
    let ligne = selectionner(
        pool,
        "api_cles",
        &[("cle_hash", mysql::Value::from(hasher(cle))), ("revoque", mysql::Value::from(0i64))],
        &["id", "user_id"],
        None,
        Some(1),
    )
    .into_iter()
    .next()?;
    let id_cle = ligne.get("id").and_then(|v| v.as_i64())?;
    let uid = ligne.get("user_id").and_then(|v| v.as_i64())?;
    let u = selectionner(pool, "login", &[("id", mysql::Value::from(uid))], &["id", "nom", "email", "privilege", "vip"], None, Some(1))
        .into_iter()
        .next()?;
    let privilege = u.get("privilege").and_then(|v| v.as_i64()).unwrap_or(10);
    if !api_autorisee(pool, uid, privilege) {
        return None;
    }
    let maintenant = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    inserer_ou_modifier(pool, "api_cles", &[("derniere_utilisation", mysql::Value::from(maintenant.as_str()))], &[("id", mysql::Value::from(id_cle))]);
    let vip = crate::utils::vip_brut_json(u.get("vip"));
    let mut info = HashMap::new();
    info.insert("connecte".into(), json!(true));
    info.insert("id".into(), json!(uid));
    info.insert("nom".into(), u.get("nom").cloned().unwrap_or(json!("")));
    info.insert("email".into(), u.get("email").cloned().unwrap_or(json!("")));
    info.insert("privilege".into(), json!(privilege));
    info.insert("vip".into(), json!(crate::utils::vip_actif(&vip)));
    info.insert("plan".into(), json!(crate::utils::plan_depuis_vip(&vip)));
    Some(info)
}

// ── Cle API = session du compte sur toutes les routes /api/ ──────
// Demande utilisateur : "qu'elle puisse faire tout ce qu'un humain peut
// faire, y compris l'administration et l'administration des extensions".
// main.rs pose ici l'identite de la cle au debut de chaque requete /api/
// (boucle principale mono-thread : une requete a la fois) et l'efface a la
// fin ; c::verifier_session et appeldb::verifier_connexion(_avec_expiration)
// la renvoient alors comme une session normale. Memes droits que le compte
// (role, permissions du panel, plan). Exception : la gestion des cles
// elle-meme (une cle ne peut pas creer/revoquer de cles).
thread_local! {
    static IDENTITE_API: std::cell::RefCell<Option<HashMap<String, Value>>> = const { std::cell::RefCell::new(None) };
}

pub fn definir_identite_api(v: Option<HashMap<String, Value>>) {
    IDENTITE_API.with(|c| *c.borrow_mut() = v);
}

pub fn identite_api() -> Option<HashMap<String, Value>> {
    IDENTITE_API.with(|c| c.borrow().clone())
}

/// Identite a poser pour cette requete : cle API valide sur une route /api/
/// (hors gestion des cles et flux d'appareil).
pub fn identite_pour(pool: &DbPool, req: &tiny_http::Request, path: &str) -> Option<HashMap<String, Value>> {
    if !path.starts_with("/api/") || path.starts_with("/api/account/cles") || path.starts_with("/api/appareil") {
        return None;
    }
    bearer(req).filter(|k| k.starts_with("vex_")).and_then(|k| verifier(pool, &k))
}

/// Jeton "Bearer ..." d'une requete.
pub fn bearer(req: &tiny_http::Request) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case("Authorization"))
        .and_then(|h| h.value.as_str().strip_prefix("Bearer ").map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// GET /api/moi (cle API) : pour tester une cle.
pub fn moi(pool: &DbPool, req: &tiny_http::Request) -> (u16, Value) {
    match bearer(req).and_then(|c| verifier(pool, &c)) {
        Some(u) => (200, json!({
            "success": true,
            "id": u.get("id"), "nom": u.get("nom"), "email": u.get("email"),
            "privilege": u.get("privilege"), "plan": u.get("plan"),
        })),
        None => (401, json!({"success": false, "error": "Clé API absente, invalide ou révoquée."})),
    }
}
