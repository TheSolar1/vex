// ══════════════════════════════════════════════════════════════════
// fchier/corbeille.rs — Corbeille ExoDrive
//
// Supprimer un fichier ou un dossier le deplace ici au lieu de l'effacer :
//   - les lignes d'origine (`fichiers` / `sitecdos`) sont copiees telles
//     quelles en JSON dans `fchier_corbeille.donnees`, puis retirees de
//     leur table ;
//   - un dossier part AVEC son contenu (sous-dossiers + fichiers de
//     l'utilisateur) -- avant, supprimer un dossier laissait ses fichiers
//     orphelins, qui reapparaissaient a la racine ;
//   - la restauration reinsere les lignes avec leurs ids d'origine, donc
//     les liens "fich:<id>" / "dos:<id>" des dossiers restent valides ;
//   - la purge (manuelle, ou automatique apres JOURS_CORBEILLE jours)
//     efface aussi les fichiers stockes sur disque ("DISK:<chemin>"), qui
//     n'etaient jamais supprimes auparavant.
// Les pages Sitec ne passent pas par la corbeille (comportement inchange).
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{
    corbeille_ids_expires, inserer_avec_erreur, selectionner, supprimer_ligne, DbPool,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub const JOURS_CORBEILLE: u32 = 30;
const PREFIXE_DISQUE: &str = "DISK:";

const COLS_FICHIERS: &[&str] = &[
    "id", "nom", "fichier", "type_fichier", "taille", "visble", "id_utilisateur", "partage", "date",
];
const COLS_DOSSIERS: &[&str] = &[
    "iddosier", "doisernom", "userid", "popluardose", "idpage", "addpageuserid",
];

type Ligne = HashMap<String, Value>;

fn parent_de(idpage: &str) -> Option<i64> {
    idpage
        .split(',')
        .find_map(|t| t.trim().strip_prefix("dos:").and_then(|v| v.parse::<i64>().ok()))
}

fn fichiers_de(idpage: &str) -> Vec<i64> {
    idpage
        .split(',')
        .filter_map(|t| t.trim().strip_prefix("fich:").and_then(|v| v.parse::<i64>().ok()))
        .collect()
}

fn ligne_vers_json(l: &Ligne) -> Value {
    Value::Object(l.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

fn json_vers_mysql(v: &Value) -> mysql::Value {
    match v {
        Value::Null => mysql::Value::NULL,
        Value::Bool(b) => mysql::Value::from(*b as i64),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                mysql::Value::from(i)
            } else if let Some(u) = n.as_u64() {
                mysql::Value::from(u)
            } else {
                mysql::Value::from(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => mysql::Value::from(s.as_str()),
        autre => mysql::Value::from(autre.to_string()),
    }
}

fn taille_de(l: &Ligne) -> i64 {
    l.get("taille").and_then(|v| v.as_i64()).unwrap_or(0)
}

/// Deplace un fichier ou un dossier (avec son contenu) dans la corbeille.
/// Erreur = (statut HTTP, message).
pub fn mettre_en_corbeille(
    pool: &DbPool,
    uid: i64,
    item_type: &str,
    item_id: i64,
) -> Result<(), (u16, String)> {
    let (nom, dossiers, fichiers): (String, Vec<Ligne>, Vec<Ligne>) = match item_type {
        "folder" => {
            let mes_dossiers = selectionner(
                pool,
                "sitecdos",
                &[("userid", mysql::Value::from(uid))],
                &[],
                None,
                None,
            );
            let racine = mes_dossiers
                .iter()
                .find(|d| d.get("iddosier").and_then(|v| v.as_i64()) == Some(item_id))
                .cloned()
                .ok_or((403, "Non autorisé".to_string()))?;
            // Sous-arbre : tous les dossiers de l'utilisateur dont un
            // ancetre est `item_id`.
            let mut dans_arbre: HashSet<i64> = HashSet::from([item_id]);
            loop {
                let avant = dans_arbre.len();
                for d in &mes_dossiers {
                    let id = d.get("iddosier").and_then(|v| v.as_i64()).unwrap_or(0);
                    let idpage = d.get("idpage").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(p) = parent_de(idpage) {
                        if dans_arbre.contains(&p) {
                            dans_arbre.insert(id);
                        }
                    }
                }
                if dans_arbre.len() == avant {
                    break;
                }
            }
            let dossiers: Vec<Ligne> = mes_dossiers
                .into_iter()
                .filter(|d| {
                    d.get("iddosier")
                        .and_then(|v| v.as_i64())
                        .map(|id| dans_arbre.contains(&id))
                        .unwrap_or(false)
                })
                .collect();
            let mut fichiers = Vec::new();
            for d in &dossiers {
                let idpage = d.get("idpage").and_then(|v| v.as_str()).unwrap_or("");
                for fid in fichiers_de(idpage) {
                    let rows = selectionner(
                        pool,
                        "fichiers",
                        &[
                            ("id", mysql::Value::from(fid)),
                            ("id_utilisateur", mysql::Value::from(uid)),
                        ],
                        &[],
                        None,
                        Some(1),
                    );
                    fichiers.extend(rows);
                }
            }
            let nom = racine
                .get("doisernom")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (nom, dossiers, fichiers)
        }
        _ => {
            let rows = selectionner(
                pool,
                "fichiers",
                &[
                    ("id", mysql::Value::from(item_id)),
                    ("id_utilisateur", mysql::Value::from(uid)),
                ],
                &[],
                None,
                Some(1),
            );
            let f = rows.into_iter().next().ok_or((403, "Non autorisé".to_string()))?;
            let nom = f.get("nom").and_then(|v| v.as_str()).unwrap_or("").to_string();
            (nom, vec![], vec![f])
        }
    };

    let taille: i64 = fichiers.iter().map(taille_de).sum();
    let donnees = json!({
        "dossiers": dossiers.iter().map(ligne_vers_json).collect::<Vec<_>>(),
        "fichiers": fichiers.iter().map(ligne_vers_json).collect::<Vec<_>>(),
    });
    let nom_court: String = nom.chars().take(250).collect();
    inserer_avec_erreur(
        pool,
        "fchier_corbeille",
        &[
            ("id_utilisateur", mysql::Value::from(uid)),
            ("item_type", mysql::Value::from(if item_type == "folder" { "folder" } else { "file" })),
            ("item_id", mysql::Value::from(item_id)),
            ("nom", mysql::Value::from(nom_court.as_str())),
            ("taille", mysql::Value::from(taille)),
            ("donnees", mysql::Value::from(donnees.to_string())),
        ],
    )
    .map_err(|e| (500, format!("Mise en corbeille impossible : {}", e)))?;

    // Copie reussie : on retire les originaux.
    for f in &fichiers {
        if let Some(id) = f.get("id").and_then(|v| v.as_i64()) {
            supprimer_ligne(pool, "fichiers", "id", mysql::Value::from(id));
        }
    }
    for d in &dossiers {
        if let Some(id) = d.get("iddosier").and_then(|v| v.as_i64()) {
            supprimer_ligne(pool, "sitecdos", "iddosier", mysql::Value::from(id));
        }
    }
    Ok(())
}

fn charger_entree(pool: &DbPool, uid: i64, id: i64) -> Option<Ligne> {
    selectionner(
        pool,
        "fchier_corbeille",
        &[
            ("id", mysql::Value::from(id)),
            ("id_utilisateur", mysql::Value::from(uid)),
        ],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()
}

fn donnees_de(entree: &Ligne) -> Value {
    entree
        .get("donnees")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!({}))
}

fn reinserer(pool: &DbPool, table: &str, cols: &[&str], ligne: &Value) -> Result<(), String> {
    let obj = ligne.as_object().ok_or("Donnees corrompues")?;
    let valeurs: Vec<(&str, mysql::Value)> = cols
        .iter()
        .filter_map(|c| obj.get(*c).map(|v| (*c, json_vers_mysql(v))))
        .collect();
    inserer_avec_erreur(pool, table, &valeurs).map(|_| ())
}

/// Restaure une entree de corbeille (lignes reinserees avec leurs ids
/// d'origine). Si une reinsertion echoue (id reutilise entre-temps), les
/// lignes deja reinserees sont retirees et l'entree reste en corbeille.
pub fn restaurer(pool: &DbPool, uid: i64, id: i64) -> Result<(), (u16, String)> {
    let entree = charger_entree(pool, uid, id).ok_or((404, "Élément introuvable".to_string()))?;
    let d = donnees_de(&entree);
    let vide = vec![];
    let dossiers = d.get("dossiers").and_then(|v| v.as_array()).unwrap_or(&vide);
    let fichiers = d.get("fichiers").and_then(|v| v.as_array()).unwrap_or(&vide);

    let mut faits: Vec<(&str, &str, i64)> = Vec::new();
    let mut erreur: Option<String> = None;
    for (table, cols, cle, lignes) in [
        ("sitecdos", COLS_DOSSIERS, "iddosier", dossiers),
        ("fichiers", COLS_FICHIERS, "id", fichiers),
    ] {
        for l in lignes {
            match reinserer(pool, table, cols, l) {
                Ok(()) => {
                    if let Some(i) = l.get(cle).and_then(|v| v.as_i64()) {
                        faits.push((table, cle, i));
                    }
                }
                Err(e) => {
                    erreur = Some(e);
                    break;
                }
            }
        }
        if erreur.is_some() {
            break;
        }
    }
    if let Some(e) = erreur {
        for (table, cle, i) in faits {
            supprimer_ligne(pool, table, cle, mysql::Value::from(i));
        }
        return Err((409, format!("Restauration impossible : {}", e)));
    }
    supprimer_ligne(pool, "fchier_corbeille", "id", mysql::Value::from(id));
    Ok(())
}

/// Supprime definitivement une entree (et ses fichiers sur disque).
pub fn purger(pool: &DbPool, uid: i64, id: i64) -> bool {
    let entree = match charger_entree(pool, uid, id) {
        Some(e) => e,
        None => return false,
    };
    let d = donnees_de(&entree);
    if let Some(fichiers) = d.get("fichiers").and_then(|v| v.as_array()) {
        for f in fichiers {
            if let Some(chemin) = f
                .get("fichier")
                .and_then(|v| v.as_str())
                .and_then(|s| s.strip_prefix(PREFIXE_DISQUE))
            {
                let _ = std::fs::remove_file(chemin);
            }
        }
    }
    supprimer_ligne(pool, "fchier_corbeille", "id", mysql::Value::from(id))
}

/// Purge automatique des elements plus vieux que JOURS_CORBEILLE jours.
pub fn purger_expires(pool: &DbPool, uid: i64) {
    for id in corbeille_ids_expires(pool, uid, JOURS_CORBEILLE) {
        purger(pool, uid, id);
    }
}

/// Liste de la corbeille (sans les donnees, potentiellement lourdes).
pub fn lister(pool: &DbPool, uid: i64) -> Value {
    purger_expires(pool, uid);
    let rows = selectionner(
        pool,
        "fchier_corbeille",
        &[("id_utilisateur", mysql::Value::from(uid))],
        &["id", "item_type", "nom", "taille", "supprime_le"],
        Some("supprime_le DESC"),
        Some(500),
    );
    let maintenant = chrono::Utc::now().naive_utc();
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            let supprime_le = r.get("supprime_le").and_then(|v| v.as_str()).unwrap_or("");
            let jours_restants = chrono::NaiveDateTime::parse_from_str(supprime_le, "%Y-%m-%d %H:%M:%S")
                .map(|t| {
                    let ecoules = (maintenant - t).num_days();
                    (JOURS_CORBEILLE as i64 - ecoules).max(0)
                })
                .unwrap_or(JOURS_CORBEILLE as i64);
            json!({
                "id": r.get("id").cloned().unwrap_or(json!(0)),
                "type": r.get("item_type").cloned().unwrap_or(json!("file")),
                "nom": r.get("nom").cloned().unwrap_or(json!("")),
                "taille": r.get("taille").cloned().unwrap_or(json!(0)),
                "supprime_le": supprime_le,
                "jours_restants": jours_restants,
            })
        })
        .collect();
    json!({"success": true, "items": items, "jours": JOURS_CORBEILLE})
}

/// Vide toute la corbeille de l'utilisateur. Renvoie le nombre d'elements.
pub fn vider(pool: &DbPool, uid: i64) -> usize {
    let ids: Vec<i64> = selectionner(
        pool,
        "fchier_corbeille",
        &[("id_utilisateur", mysql::Value::from(uid))],
        &["id"],
        None,
        None,
    )
    .iter()
    .filter_map(|r| r.get("id").and_then(|v| v.as_i64()))
    .collect();
    ids.iter().filter(|id| purger(pool, uid, **id)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_idpage() {
        assert_eq!(parent_de("dos:4,fich:7,fich:9"), Some(4));
        assert_eq!(parent_de("fich:7"), None);
        assert_eq!(fichiers_de("dos:4, fich:7,page:ab,fich:9"), vec![7, 9]);
    }

    #[test]
    fn conversion_json_mysql() {
        assert_eq!(json_vers_mysql(&json!(5)), mysql::Value::Int(5));
        assert_eq!(json_vers_mysql(&Value::Null), mysql::Value::NULL);
        assert_eq!(json_vers_mysql(&json!("a")), mysql::Value::Bytes(b"a".to_vec()));
    }
}

/// Test de bout en bout contre une vraie base (MariaDB/MySQL) :
///   VEX_TEST_DB=mysql://user:mdp@127.0.0.1:3306/vex_test cargo test -- --ignored corbeille
/// La base est creee/initialisee par db_init.
#[cfg(test)]
mod tests_db {
    use super::*;
    use crate::appeldb::inserer_ou_modifier;

    fn pool_test() -> Option<DbPool> {
        let url = std::env::var("VEX_TEST_DB").ok()?;
        let opts = mysql::Opts::from_url(&url).ok()?;
        let cfg = crate::config_loader::DbConfig {
            host: opts.get_ip_or_hostname().to_string(),
            user: opts.get_user()?.to_string(),
            password: opts.get_pass().unwrap_or("").to_string(),
            dbname: opts.get_db_name()?.to_string(),
            port: opts.get_tcp_port(),
        };
        crate::db_init::init_db(&cfg).ok()?;
        crate::appeldb::creer_pool(&cfg).ok()
    }

    fn compter(pool: &DbPool, table: &str, col: &str, id: i64) -> usize {
        selectionner(pool, table, &[(col, mysql::Value::from(id))], &[], None, None).len()
    }

    #[test]
    #[ignore]
    fn corbeille_dossier_aller_retour() {
        let pool = match pool_test() {
            Some(p) => p,
            None => { eprintln!("VEX_TEST_DB absent -- test ignore"); return; }
        };
        let uid = 900_000 + (std::process::id() as i64 % 10_000);
        let dossier_disque = std::env::temp_dir().join(format!("vex_corbeille_{}", uid));
        std::fs::create_dir_all(&dossier_disque).unwrap();
        let fichier_disque = dossier_disque.join("a.bin");
        std::fs::write(&fichier_disque, b"x").unwrap();

        let f1 = inserer_ou_modifier(&pool, "fichiers", &[
            ("nom", mysql::Value::from("a.txt")),
            ("fichier", mysql::Value::from(format!("DISK:{}", fichier_disque.display()))),
            ("type_fichier", mysql::Value::from("text/plain")),
            ("taille", mysql::Value::from(10i64)),
            ("visble", mysql::Value::from("prive")),
            ("id_utilisateur", mysql::Value::from(uid.to_string())),
        ], &[]);
        let d1 = inserer_ou_modifier(&pool, "sitecdos", &[
            ("doisernom", mysql::Value::from("Parent")),
            ("userid", mysql::Value::from(uid.to_string())),
            ("idpage", mysql::Value::from(format!("fich:{}", f1))),
            ("addpageuserid", mysql::Value::from("")),
        ], &[]);
        let d2 = inserer_ou_modifier(&pool, "sitecdos", &[
            ("doisernom", mysql::Value::from("Enfant")),
            ("userid", mysql::Value::from(uid.to_string())),
            ("idpage", mysql::Value::from(format!("dos:{}", d1))),
            ("addpageuserid", mysql::Value::from("")),
        ], &[]);
        assert!(f1 > 0 && d1 > 0 && d2 > 0);

        // Un autre utilisateur ne peut pas supprimer ce dossier.
        assert_eq!(mettre_en_corbeille(&pool, uid + 1, "folder", d1).unwrap_err().0, 403);

        mettre_en_corbeille(&pool, uid, "folder", d1).unwrap();
        assert_eq!(compter(&pool, "fichiers", "id", f1), 0);
        assert_eq!(compter(&pool, "sitecdos", "iddosier", d1), 0);
        assert_eq!(compter(&pool, "sitecdos", "iddosier", d2), 0, "sous-dossier emporte");

        let liste = lister(&pool, uid);
        let items = liste["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["nom"], "Parent");
        assert_eq!(items[0]["taille"], 10);
        assert_eq!(items[0]["jours_restants"], JOURS_CORBEILLE as i64);
        let cid = items[0]["id"].as_i64().unwrap();

        restaurer(&pool, uid, cid).unwrap();
        assert_eq!(compter(&pool, "fichiers", "id", f1), 1, "fichier restaure avec son id");
        assert_eq!(compter(&pool, "sitecdos", "iddosier", d1), 1);
        assert_eq!(compter(&pool, "sitecdos", "iddosier", d2), 1);
        assert!(lister(&pool, uid)["items"].as_array().unwrap().is_empty());

        // Fichier seul -> corbeille -> purge definitive (fichier disque efface).
        mettre_en_corbeille(&pool, uid, "file", f1).unwrap();
        assert_eq!(vider(&pool, uid), 1);
        assert!(!fichier_disque.exists(), "fichier disque supprime a la purge");

        for d in [d1, d2] { supprimer_ligne(&pool, "sitecdos", "iddosier", mysql::Value::from(d)); }
        let _ = std::fs::remove_dir_all(&dossier_disque);
    }
}
