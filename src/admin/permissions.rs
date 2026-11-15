// ══════════════════════════════════════════════════════════════════
// admin/permissions.rs — permissions du panel admin par privilege.
//
// Demandes utilisateur : "pouvoir enlever/ajouter des permissions dans le
// panel admin aux differents privileges si on est fondateur", puis
// "editer plus precisement les permissions" (action par action).
//
// - Le fondateur (privilege 1) a TOUJOURS tout, et lui seul peut modifier
//   ces permissions (route /api/admin/permissions).
// - Stockage : config.json -> "admin_permissions": { "2": [..], "3": [..] }.
//   Un privilege absent de cette table garde les regles historiques
//   (permissions_par_defaut) : rien ne change tant que le fondateur n'a
//   rien enregistre. Les anciens ids (une permission par section) sont
//   convertis a la lecture (ANCIENS_IDS).
// - /api/admin/config (POST) : chaque cle de config.json modifiee exige sa
//   permission (verifier_ecriture_config) et ne peut JAMAIS reecrire
//   admin_permissions.
// ══════════════════════════════════════════════════════════════════

use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};

/// Permission : (id, libelle, sensible). "Sensible" = execution de code,
/// donnees privees ou action irreversible sur le serveur.
type Perm = (&'static str, &'static str, bool);

/// Groupes affiches dans la grille : (libelle, icone /static/img/solid/, permissions).
pub(crate) const GROUPES: &[(&str, &str, &[Perm])] = &[
    ("Utilisateurs", "users", &[
        ("users_voir", "Voir la liste des comptes", false),
        ("users_role", "Changer le rôle d'un compte", false),
        ("users_plan", "Changer le plan (VIP) d'un compte", false),
        ("users_supprimer", "Supprimer un compte", true),
    ]),
    ("Blocages", "ban", &[
        ("blocks_voir", "Voir les blocages", false),
        ("blocks_modifier", "Ajouter / retirer un blocage", false),
    ]),
    ("Base de données", "database", &[
        ("db_voir", "Lire les tables", true),
        ("db_modifier", "Modifier / ajouter / supprimer des lignes", true),
        ("db_sql", "Exécuter du SQL", true),
    ]),
    ("Configuration, rôles et plans", "gear", &[
        ("config_voir", "Voir la configuration et le serveur", false),
        ("roles_noms", "Renommer les rôles", false),
        ("plans_modifier", "Modifier les plans (prix, quotas, codes promo)", false),
        ("config_generale", "Modifier le reste de la configuration", true),
    ]),
    ("Applications et extensions", "puzzle-piece", &[
        ("extensions_voir", "Voir les applications et extensions", false),
        ("extensions_acces", "Régler qui a accès à chaque application", false),
        ("extensions_code", "Installer / mettre à jour / compiler", true),
    ]),
    ("Revenus", "euro-sign", &[
        ("revenus_voir", "Voir les revenus", false),
    ]),
    ("Logs et surveillance", "scroll", &[
        ("logs_voir", "Voir les logs", false),
        ("logs_vider", "Vider les logs", true),
        ("empreintes", "Voir les empreintes d'appareil", true),
        ("audit_vexia", "Voir l'audit des actions VexIA", true),
    ]),
    ("Éditeur en ligne", "file-lines", &[
        ("editeur_voir", "Voir l'état de l'éditeur", false),
        ("editeur_configurer", "Choisir / configurer l'éditeur", true),
        ("editeur_controle", "Démarrer / arrêter l'éditeur", true),
    ]),
    ("P2P / AnonNet", "network-wired", &[
        ("p2p_voir", "Voir les nœuds et transferts", false),
        ("p2p_gerer", "Exclure des nœuds, synchroniser", true),
    ]),
    ("Sauvegardes", "box-archive", &[
        ("backup_voir", "Voir les sauvegardes", false),
        ("backup_creer", "Créer / programmer une sauvegarde", true),
        ("backup_telecharger", "Télécharger une sauvegarde (base complète)", true),
        ("backup_supprimer", "Supprimer une sauvegarde", true),
    ]),
    ("Machine", "server", &[
        ("machine_voir", "Voir l'état et les versions", false),
        ("machine_maj", "Mettre à jour / revenir en arrière / redémarrer VEX", true),
        ("machine_os", "Mettre à jour / redémarrer le système", true),
        ("machine_stockage", "Gérer le stockage disque", true),
    ]),
];

/// Anciens ids (une permission par section) -> nouveaux ids.
const ANCIENS_IDS: &[(&str, &[&str])] = &[
    ("users", &["users_voir", "users_role", "users_plan", "users_supprimer"]),
    ("blocks", &["blocks_voir", "blocks_modifier"]),
    ("db", &["db_voir"]),
    ("db_ecriture", &["db_modifier", "db_sql"]),
    ("config", &["config_voir"]),
    ("config_ecriture", &["roles_noms", "plans_modifier", "config_generale"]),
    ("extensions", &["extensions_voir", "extensions_acces"]),
    ("revenus", &["revenus_voir"]),
    ("logs", &["logs_voir", "logs_vider"]),
    ("editeur", &["editeur_voir", "editeur_configurer"]),
    ("p2p", &["p2p_voir", "p2p_gerer"]),
    ("backup", &["backup_voir", "backup_creer", "backup_telecharger", "backup_supprimer"]),
    ("machine", &["machine_voir", "machine_maj", "machine_os", "machine_stockage"]),
];

fn toutes() -> impl Iterator<Item = &'static Perm> {
    GROUPES.iter().flat_map(|g| g.2.iter())
}

fn est_connue(id: &str) -> bool {
    toutes().any(|p| p.0 == id)
}

fn libelle(id: &str) -> &'static str {
    toutes().find(|p| p.0 == id).map(|p| p.1).unwrap_or("?")
}

/// Permission declaree par une application externe (WFront...) : "app.nom".
/// VEX ne l'applique pas lui-meme : il la stocke dans les memes tables et
/// c'est l'application qui la lit dans config.json.
fn est_externe(id: &str) -> bool {
    let mut it = id.splitn(2, '.');
    let (a, b) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
    !a.is_empty()
        && !b.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '-')
}

/// Au moins une permission du panel VEX (les permissions d'applications
/// externes seules ne donnent pas acces au panel).
pub(crate) fn acces_panel_vex(p: &HashSet<String>) -> bool {
    p.iter().any(|x| est_connue(x))
}

/// Ids valides correspondant a une entree enregistree (ancien id converti).
fn developper(id: &str) -> Vec<String> {
    if est_connue(id) || est_externe(id) {
        return vec![id.to_string()];
    }
    ANCIENS_IDS
        .iter()
        .find(|(ancien, _)| *ancien == id)
        .map(|(_, n)| n.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
}

/// Regles historiques : 2 = super admin = tout sauf Machine ; 3 = admin =
/// sections non sensibles ; 4 et plus = aucun acces.
fn permissions_par_defaut(privilege: i64) -> Vec<&'static str> {
    match privilege {
        2 => toutes().map(|p| p.0).filter(|p| !p.starts_with("machine_")).collect(),
        3 => vec![
            "users_voir", "users_role", "users_plan", "users_supprimer",
            "blocks_voir", "blocks_modifier", "db_voir", "config_voir",
            "extensions_voir", "extensions_acces", "revenus_voir",
            "logs_voir", "logs_vider", "editeur_voir", "editeur_configurer",
        ],
        _ => vec![],
    }
}

/// Permissions effectives d'un privilege, lues depuis la config brute.
pub(crate) fn permissions_de(cfg: &Value, privilege: i64) -> HashSet<String> {
    if privilege == 1 {
        return toutes().map(|p| p.0.to_string()).collect();
    }
    match cfg.get("admin_permissions").and_then(|t| t.get(privilege.to_string())).and_then(|v| v.as_array()) {
        Some(liste) => liste.iter().filter_map(|v| v.as_str()).flat_map(developper).collect(),
        None => permissions_par_defaut(privilege).into_iter().map(|s| s.to_string()).collect(),
    }
}

/// Permissions en plus donnees a UN compte (config.json ->
/// "admin_permissions_comptes": { "<id>": [..] }), ajoutees a celles du role.
fn permissions_en_plus(cfg: &Value, user_id: i64) -> HashSet<String> {
    cfg.get("admin_permissions_comptes")
        .and_then(|t| t.get(user_id.to_string()))
        .and_then(|v| v.as_array())
        .map(|l| l.iter().filter_map(|v| v.as_str()).flat_map(developper).collect())
        .unwrap_or_default()
}

/// Permissions effectives d'un compte : celles de son role + les siennes.
pub(crate) fn permissions_compte(cfg: &Value, privilege: i64, user_id: i64) -> HashSet<String> {
    let mut p = permissions_de(cfg, privilege);
    p.extend(permissions_en_plus(cfg, user_id));
    p
}

/// Liste triee (reponse JSON stable).
pub(crate) fn triees(p: &HashSet<String>) -> Vec<String> {
    let mut l: Vec<String> = p.iter().cloned().collect();
    l.sort();
    l
}

/// Lit config.json (chemin par defaut) -- pour le menu de navigation.
pub(crate) fn a_acces_panel(privilege: i64, user_id: i64) -> bool {
    let cfg = std::fs::read_to_string("config.json")
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(json!({}));
    acces_panel_vex(&permissions_compte(&cfg, privilege, user_id))
}

/// Blocs declares par les applications externes. config.json ->
/// "permissions_apps": ["http://127.0.0.1:8095/api/permissions", ...] ;
/// chaque adresse renvoie {"groupes":[{"label","icon","permissions":[{"id":"app.x","label","danger"}]}]}.
/// Adresses locales uniquement ; delai court (VEX traite une requete a la
/// fois, et seul le fondateur ouvre cette grille).
fn groupes_externes(cfg: &Value) -> Vec<Value> {
    let urls: Vec<String> = cfg
        .get("permissions_apps")
        .and_then(|v| v.as_array())
        .map(|l| l.iter().filter_map(|u| u.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if urls.is_empty() {
        return vec![];
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_millis(800))
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let mut out = vec![];
    for u in urls.iter().take(10) {
        if !(u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:")) {
            continue;
        }
        let rep: Option<Value> = agent.get(u).call().ok().and_then(|r| r.into_json().ok());
        let rep = match rep {
            Some(v) => v,
            None => {
                out.push(json!({"label": format!("Application injoignable ({})", u), "icon": "triangle-exclamation", "externe": true, "permissions": []}));
                continue;
            }
        };
        for g in rep.get("groupes").and_then(|g| g.as_array()).into_iter().flatten().take(10) {
            let label: String = g.get("label").and_then(|v| v.as_str()).unwrap_or("Application").chars().take(60).collect();
            let icon = g
                .get("icon")
                .and_then(|v| v.as_str())
                .filter(|i| !i.is_empty() && i.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
                .filter(|i| std::path::Path::new(&format!("static/img/solid/{}.svg", i)).exists())
                .unwrap_or("puzzle-piece");
            let perms: Vec<Value> = g
                .get("permissions")
                .and_then(|p| p.as_array())
                .into_iter()
                .flatten()
                .take(50)
                .filter_map(|p| {
                    let id = p.get("id")?.as_str()?;
                    if !est_externe(id) {
                        return None;
                    }
                    let l: String = p.get("label").and_then(|v| v.as_str()).unwrap_or(id).chars().take(100).collect();
                    Some(json!({"id": id, "label": l, "danger": p.get("danger").and_then(|v| v.as_bool()).unwrap_or(false)}))
                })
                .collect();
            if !perms.is_empty() {
                out.push(json!({"label": label, "icon": icon, "externe": true, "permissions": perms}));
            }
        }
    }
    out
}

/// Permission requise par une route /api/admin/<sub>. None = tout
/// utilisateur ayant acces au panel (tableau de bord, lecture de config
/// filtree, ecriture de config verifiee cle par cle dans le handler).
/// Some("fondateur") = reserve au fondateur.
pub(crate) fn permission_requise(sub: &str, method: &str) -> Option<&'static str> {
    let commence = |p: &str| sub == p || sub.starts_with(&format!("{}/", p));
    let post = method == "POST";
    let r = match sub {
        "/dashboard" | "/config" => return None,
        _ if commence("/permissions") => "fondateur",
        // Utilisateurs
        "/users/privilege" => "users_role",
        "/users/vip" => "users_plan",
        "/users/delete" => "users_supprimer",
        _ if commence("/users") => "users_voir",
        // Surveillance
        _ if commence("/empreintes") => "empreintes",
        _ if commence("/vexia/audit") => "audit_vexia",
        _ if commence("/revenus_stats") => "revenus_voir",
        // Blocages
        "/blocks/add" | "/blocks/delete" => "blocks_modifier",
        _ if commence("/blocks") || commence("/pages") => "blocks_voir",
        // Base de donnees
        _ if commence("/db/sql") => "db_sql",
        _ if commence("/db/cell") || commence("/db/row") => "db_modifier",
        _ if commence("/db") => "db_voir",
        // P2P
        _ if commence("/p2p") => if post { "p2p_gerer" } else { "p2p_voir" },
        // Sauvegardes
        "/backup/list" | "/backup/status" => "backup_voir",
        "/backup/auto" => if post { "backup_creer" } else { "backup_voir" },
        "/backup/create" => "backup_creer",
        "/backup/download" => "backup_telecharger",
        "/backup/delete" => "backup_supprimer",
        _ if commence("/backup") => "backup_creer",
        // Machine
        "/machine/status" | "/machine/check" | "/machine/update_status" | "/machine/versions"
        | "/machine/os_update_status" => "machine_voir",
        "/machine/os_update" | "/machine/os_restart_schedule" | "/machine/os_restart_cancel" => "machine_os",
        _ if commence("/machine/disk_storage") => "machine_stockage",
        _ if commence("/machine") => "machine_maj",
        // Logs
        "/logs/clear" => "logs_vider",
        _ if commence("/logs") => "logs_voir",
        // Editeur en ligne
        "/onlyoffice/start" | "/onlyoffice/stop" | "/editor/start" | "/editor/stop" => "editeur_controle",
        "/editor/select" | "/editor/add" | "/editor/save" | "/editor/delete" => "editeur_configurer",
        _ if commence("/onlyoffice") || commence("/editor") => "editeur_voir",
        // Applications / extensions
        "/extensions/permissions" => "extensions_acces",
        "/extensions/rebuild/status" | "/extensions/template" => "extensions_voir",
        "/extensions/upload" | "/extensions/delete" | "/extensions/rebuild" | "/extensions/update"
        | "/marketplace/install" | "/marketplace/source" => "extensions_code",
        _ if commence("/extensions") || commence("/marketplace") => "extensions_voir",
        _ if commence("/server") => "config_voir",
        // Route inconnue de cette table : on reste prudent.
        _ => "config_generale",
    };
    Some(r)
}

const MASQUE: &str = "••••••";

/// Config renvoyee a un compte sans "config_generale" : secrets masques
/// (cles, mots de passe, jetons).
pub(crate) fn masquer_secrets(v: &mut Value) {
    match v {
        Value::Object(m) => {
            for (k, val) in m.iter_mut() {
                let kl = k.to_lowercase();
                let sensible = ["secret", "password", "passwd", "motdepasse", "token", "api_key", "apikey", "private"]
                    .iter()
                    .any(|s| kl.contains(s));
                if sensible && (val.is_string() || val.is_number()) {
                    *val = json!(MASQUE);
                } else {
                    masquer_secrets(val);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(masquer_secrets),
        _ => {}
    }
}

/// Remet les vraies valeurs a la place des secrets masques renvoyes tels
/// quels par le navigateur.
fn restaurer_masques(nouveau: &mut Value, actuel: &Value) {
    match nouveau {
        Value::String(s) if s == MASQUE => {
            if !actuel.is_null() {
                *nouveau = actuel.clone();
            }
        }
        Value::Object(m) => {
            for (k, val) in m.iter_mut() {
                restaurer_masques(val, actuel.get(k).unwrap_or(&Value::Null));
            }
        }
        Value::Array(a) => {
            for (i, val) in a.iter_mut().enumerate() {
                restaurer_masques(val, actuel.get(i).unwrap_or(&Value::Null));
            }
        }
        _ => {}
    }
}

/// Peut-on ouvrir l'enregistrement de config ? (au moins une des trois)
pub(crate) fn peut_ecrire_config(perms: &HashSet<String>) -> bool {
    ["roles_noms", "plans_modifier", "config_generale"].iter().any(|p| perms.contains(*p))
}

/// POST /config : prepare `nouveau` (secrets masques restaures, table des
/// permissions conservee) puis verifie que chaque cle modifiee est permise.
pub(crate) fn verifier_ecriture_config(perms: &HashSet<String>, actuel: &Value, nouveau: &mut Value) -> Result<(), String> {
    restaurer_masques(nouveau, actuel);
    if let Some(obj) = nouveau.as_object_mut() {
        for cle in ["admin_permissions", "admin_permissions_comptes"] {
            match actuel.get(cle) {
                Some(p) => {
                    obj.insert(cle.into(), p.clone());
                }
                None => {
                    obj.remove(cle);
                }
            }
        }
    }
    let vide = serde_json::Map::new();
    let a = actuel.as_object().unwrap_or(&vide);
    let n = nouveau.as_object().ok_or("Configuration invalide.")?;
    let cles: BTreeSet<&String> = a.keys().chain(n.keys()).collect();
    let mut manquantes = BTreeSet::new();
    for k in cles {
        if a.get(k) == n.get(k) {
            continue;
        }
        let requise = match k.as_str() {
            "privilege_labels" => "roles_noms",
            "plans" => "plans_modifier",
            _ => "config_generale",
        };
        if !perms.contains(requise) {
            manquantes.insert(libelle(requise));
        }
    }
    if manquantes.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Permission refusée : {}.",
            manquantes.into_iter().collect::<Vec<_>>().join(", ")
        ))
    }
}

/// GET /api/admin/permissions : catalogue groupe + table effective (2..12)
/// + permissions en plus par compte + liste des comptes (hors fondateurs).
pub(crate) fn lister(cfg: &Value, comptes_liste: Vec<Value>) -> Value {
    let mut roles = serde_json::Map::new();
    for p in 2..=12 {
        roles.insert(p.to_string(), json!(triees(&permissions_de(cfg, p))));
    }
    let mut comptes = serde_json::Map::new();
    if let Some(t) = cfg.get("admin_permissions_comptes").and_then(|t| t.as_object()) {
        for (uid, _) in t {
            if let Ok(id) = uid.parse::<i64>() {
                let p = permissions_en_plus(cfg, id);
                if !p.is_empty() {
                    comptes.insert(uid.clone(), json!(triees(&p)));
                }
            }
        }
    }
    json!({
        "success": true,
        "data": {
            "groupes": GROUPES.iter().map(|(lib, ico, perms)| json!({
                "label": lib,
                "icon": ico,
                "permissions": perms.iter().map(|(id, l, d)| json!({"id": id, "label": l, "danger": d})).collect::<Vec<_>>(),
            })).chain(groupes_externes(cfg)).collect::<Vec<_>>(),
            "roles": roles,
            "comptes": comptes,
            "comptes_liste": comptes_liste,
        }
    })
}

/// Ids valides d'une liste JSON (ids inconnus ignores), tries sans doublon.
fn ids_valides(v: &Value) -> Result<Vec<String>, String> {
    let mut ids: Vec<String> = v
        .as_array()
        .ok_or("Format invalide.")?
        .iter()
        .filter_map(|x| x.as_str())
        .filter(|id| est_connue(id) || est_externe(id))
        .map(|s| s.to_string())
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// POST /api/admin/permissions (fondateur) : body "permissions" = JSON
/// { "2": ["users_voir", ...], ... } et, optionnel, "comptes" = JSON
/// { "<id>": [...] } (permissions en plus par compte ; liste vide = retiree).
/// Ids inconnus ignores, privileges hors 2..12 refuses (le fondateur garde
/// tout, toujours).
pub(crate) fn enregistrer(config_path: &str, brut: &str, brut_comptes: Option<&str>) -> Result<(), String> {
    let entree: Value = serde_json::from_str(brut).map_err(|_| "JSON invalide.".to_string())?;
    let entree = entree.as_object().ok_or("Format invalide.")?;
    let mut table = serde_json::Map::new();
    for (k, v) in entree {
        let p: i64 = k.parse().map_err(|_| format!("Privilège invalide : {}", k))?;
        if !(2..=12).contains(&p) {
            return Err(format!("Privilège invalide : {}", k));
        }
        table.insert(p.to_string(), json!(ids_valides(v)?));
    }
    let comptes = match brut_comptes.filter(|s| !s.trim().is_empty()) {
        Some(b) => {
            let e: Value = serde_json::from_str(b).map_err(|_| "JSON invalide (comptes).".to_string())?;
            let mut t = serde_json::Map::new();
            for (k, v) in e.as_object().ok_or("Format invalide (comptes).")? {
                let id: i64 = k.parse().map_err(|_| format!("Compte invalide : {}", k))?;
                if id <= 0 {
                    return Err(format!("Compte invalide : {}", k));
                }
                let ids = ids_valides(v)?;
                if !ids.is_empty() {
                    t.insert(id.to_string(), json!(ids));
                }
            }
            Some(t)
        }
        None => None,
    };
    let mut cfg: Value = std::fs::read_to_string(config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or("config.json illisible.")?;
    let obj = cfg.as_object_mut().ok_or("config.json invalide.")?;
    obj.insert("admin_permissions".into(), Value::Object(table));
    if let Some(t) = comptes {
        obj.insert("admin_permissions_comptes".into(), Value::Object(t));
    }
    std::fs::write(config_path, serde_json::to_string_pretty(&cfg).unwrap_or_default())
        .map_err(|e| format!("Écriture impossible : {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(l: &[&str]) -> HashSet<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defauts_identiques_aux_anciennes_regles() {
        let cfg = json!({});
        assert!(permissions_de(&cfg, 2).contains("db_sql"));
        assert!(!permissions_de(&cfg, 2).contains("machine_voir"));
        assert!(permissions_de(&cfg, 3).contains("users_supprimer"));
        assert!(!permissions_de(&cfg, 3).contains("config_generale"));
        assert!(!permissions_de(&cfg, 3).contains("db_modifier"));
        assert!(permissions_de(&cfg, 6).is_empty());
        assert!(permissions_de(&cfg, 1).contains("machine_os"));
    }

    #[test]
    fn anciens_ids_convertis() {
        let cfg = json!({"admin_permissions": {"6": ["logs", "inconnue"], "4": ["users_voir"], "2": []}});
        assert_eq!(permissions_de(&cfg, 6), set(&["logs_voir", "logs_vider"]));
        assert_eq!(permissions_de(&cfg, 4), set(&["users_voir"]));
        assert!(permissions_de(&cfg, 2).is_empty());
    }

    #[test]
    fn permissions_par_compte() {
        let cfg = json!({"admin_permissions": {"6": ["logs_voir"]}, "admin_permissions_comptes": {"42": ["users_voir", "inconnue"]}});
        assert_eq!(permissions_compte(&cfg, 6, 42), set(&["logs_voir", "users_voir"]));
        assert_eq!(permissions_compte(&cfg, 6, 7), set(&["logs_voir"]));
        // Un compte sans role admin (10) mais avec une permission en plus entre dans le panel.
        assert_eq!(permissions_compte(&cfg, 10, 42), set(&["users_voir"]));
        // La table par compte est protegee contre /config.
        let mut n = cfg.clone();
        n["admin_permissions_comptes"] = json!({"42": ["machine_os"]});
        assert!(verifier_ecriture_config(&set(&[]), &cfg, &mut n).is_ok());
        assert_eq!(n["admin_permissions_comptes"], cfg["admin_permissions_comptes"]);
    }

    #[test]
    fn permissions_externes() {
        assert!(est_externe("wfront.admin"));
        assert!(!est_externe("wfront"));
        assert!(!est_externe("WFront.Admin"));
        assert!(!est_externe(".admin"));
        let cfg = json!({"admin_permissions": {"6": ["wfront.admin"]}, "admin_permissions_comptes": {"9": ["wfront.admin", "logs_voir"]}});
        // Stockee et relue, mais n'ouvre pas le panel VEX a elle seule.
        assert_eq!(permissions_de(&cfg, 6), set(&["wfront.admin"]));
        assert!(!acces_panel_vex(&permissions_de(&cfg, 6)));
        assert!(acces_panel_vex(&permissions_compte(&cfg, 10, 9)));
    }

    #[test]
    fn routes() {
        assert_eq!(permission_requise("/dashboard", "GET"), None);
        assert_eq!(permission_requise("/config", "POST"), None);
        assert_eq!(permission_requise("/users", "GET"), Some("users_voir"));
        assert_eq!(permission_requise("/users/vip", "POST"), Some("users_plan"));
        assert_eq!(permission_requise("/db/table/login", "GET"), Some("db_voir"));
        assert_eq!(permission_requise("/db/sql", "POST"), Some("db_sql"));
        assert_eq!(permission_requise("/backup/auto", "GET"), Some("backup_voir"));
        assert_eq!(permission_requise("/backup/auto", "POST"), Some("backup_creer"));
        assert_eq!(permission_requise("/machine/os_update_status", "GET"), Some("machine_voir"));
        assert_eq!(permission_requise("/machine/os_update", "POST"), Some("machine_os"));
        assert_eq!(permission_requise("/extensions/rebuild", "POST"), Some("extensions_code"));
        assert_eq!(permission_requise("/extensions/rebuild/status", "GET"), Some("extensions_voir"));
        assert_eq!(permission_requise("/permissions", "POST"), Some("fondateur"));
        assert_eq!(permission_requise("/inconnue", "GET"), Some("config_generale"));
    }

    #[test]
    fn ecriture_config_par_cle() {
        let actuel = json!({"privilege_labels": {"6": "Modo"}, "plans": {"paiement_secret": "abc", "x": 1}, "app": {"name": "VEX"}, "admin_permissions": {"3": []}});
        // Renommer un role avec seulement roles_noms : OK, secret restaure, table conservee.
        let mut n = actuel.clone();
        n["privilege_labels"]["6"] = json!("Moderateur");
        n["plans"]["paiement_secret"] = json!(MASQUE);
        n["admin_permissions"] = json!({"3": ["machine_os"]});
        assert!(verifier_ecriture_config(&set(&["roles_noms"]), &actuel, &mut n).is_ok());
        assert_eq!(n["plans"]["paiement_secret"], json!("abc"));
        assert_eq!(n["admin_permissions"], json!({"3": []}));
        // Changer le nom de l'app sans config_generale : refuse.
        let mut n2 = actuel.clone();
        n2["app"]["name"] = json!("Autre");
        assert!(verifier_ecriture_config(&set(&["roles_noms", "plans_modifier"]), &actuel, &mut n2).is_err());
    }
}
