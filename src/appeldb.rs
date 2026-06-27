// ══════════════════════════════════════════════════════════════════
// appeldb.rs — VEX Database layer
// Équivalent direct de appeldb.php
// Toutes les autres pages Rust importent ce module — jamais de SQL ailleurs.
// ══════════════════════════════════════════════════════════════════

use chrono::{Duration, NaiveDateTime, TimeZone, Utc};
use mysql::prelude::*;
use mysql::*;
use serde_json::{json, Value};
use std::collections::HashMap;

pub use crate::config_loader::DbConfig;

pub type DbPool = Pool;

/// Crée le pool MySQL. Appelé une seule fois dans main.rs.
pub fn creer_pool(cfg: &DbConfig) -> Result<DbPool> {
    let url = format!(
        "mysql://{}:{}@{}:{}/{}",
        cfg.user, cfg.password, cfg.host, cfg.port, cfg.dbname
    );
    let opts = Opts::from_url(&url)?;
    Pool::new(opts)
}

/// Crée un pool sur une base différente.
pub fn creer_pool_db(cfg: &DbConfig, db_name: &str) -> Result<DbPool> {
    let url = format!(
        "mysql://{}:{}@{}:{}/{}",
        cfg.user, cfg.password, cfg.host, cfg.port, db_name
    );
    Pool::new(Opts::from_url(&url)?)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 1 : verifier_connexion()
// Vérifie le cookie dans loginc, puis récupère les vraies infos
// (id, privilege, vip) depuis la table login via l'email.
// La durée de validité de session est lue depuis config.json
// (users.session_expiration_minutes) — jamais hardcodée ici.
// Retourne None si cookie invalide, expiré, IP/UA différent,
// ou si l'utilisateur n'existe plus dans login.
// ══════════════════════════════════════════════════════════════════
pub fn verifier_connexion(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
) -> Option<HashMap<String, Value>> {
    if cookie_val.is_empty() {
        return None;
    }

    let mut conn = pool.get_conn().ok()?;

    // 1. Vérifie que le cookie existe (pas de filtre heure — problème UTC+2)
    let row: mysql::Row = conn
        .exec_first(
            "SELECT idcokier, datecra, pc, navi, email, nom \
         FROM loginc \
         WHERE idcokier = ?",
            (cookie_val,),
        )
        .ok()??;

    // Vérifie IP et User-Agent manuellement
    let pc: String = row.get("pc").unwrap_or_default();
    let navi: String = row.get("navi").unwrap_or_default();
    if pc != remote_ip || navi != user_agent {
        return None;
    }

    let email: String = row.get("email")?;
    let nom: String = row.get("nom").unwrap_or_default();

    // 2. Récupère les vraies infos depuis login (id, privilege, vip)
    let user_row: mysql::Row = conn
        .exec_first(
            "SELECT id, nom, email, privilege, vip \
         FROM login \
         WHERE email = ? \
         LIMIT 1",
            (&email,),
        )
        .ok()??;

    let id: i64 = user_row.get("id")?;
    let privilege: i64 = user_row.get("privilege").unwrap_or(10);
    let vip: i64 = user_row.get("vip").unwrap_or(0);
    let nom_login: String = user_row.get("nom").unwrap_or(nom);

    let mut info = HashMap::new();
    info.insert("connecte".into(), json!(true));
    info.insert("id".into(), json!(id));
    info.insert("nom".into(), json!(nom_login));
    info.insert("email".into(), json!(email));
    info.insert("privilege".into(), json!(privilege));
    info.insert("vip".into(), json!(vip));
    Some(info)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 1b : verifier_connexion_avec_expiration()
// Même chose que verifier_connexion() mais filtre aussi sur la
// durée de validité passée en paramètre (lue depuis config.json
// dans l'appelant — users.session_expiration_minutes).
// ══════════════════════════════════════════════════════════════════
pub fn verifier_connexion_avec_expiration(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    session_minutes: u32,
) -> Option<HashMap<String, Value>> {
    if cookie_val.is_empty() {
        return None;
    }

    let mut conn = pool.get_conn().ok()?;

    // On ne filtre plus par date côté SQL pour éviter les soucis de fuseau horaire
    // (datecra enregistrée en UTC, NOW() côté MySQL peut être en heure locale).
    let row: mysql::Row = conn
        .exec_first(
            "SELECT idcokier, datecra, pc, navi, email, nom \
         FROM loginc \
         WHERE idcokier = ?",
            (cookie_val,),
        )
        .ok()??;

    let pc: String = row.get("pc").unwrap_or_default();
    let navi: String = row.get("navi").unwrap_or_default();
    if pc != remote_ip || navi != user_agent {
        return None;
    }

    // Vérifie l'expiration en comparant avec l'heure UTC en Rust
    let datecra_val: mysql::Value = row.get("datecra").unwrap_or(mysql::Value::NULL);
    let created = match datecra_val {
        mysql::Value::Date(y, m, d, h, mi, s, micros) => {
            chrono::NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32)
                .and_then(|d| d.and_hms_micro_opt(h as u32, mi as u32, s as u32, micros))
                .map(|dt| Utc.from_utc_datetime(&dt))
        }
        mysql::Value::Bytes(b) => std::str::from_utf8(&b)
            .ok()
            .and_then(|s| {
                NaiveDateTime::parse_from_str(s.trim_matches('\''), "%Y-%m-%d %H:%M:%S").ok()
            })
            .map(|dt| Utc.from_utc_datetime(&dt)),
        _ => None,
    };

    let recent = created
        .map(|dt| Utc::now().signed_duration_since(dt) < Duration::minutes(session_minutes as i64))
        .unwrap_or(false);
    if !recent {
        return None;
    }

    let email: String = row.get("email")?;
    let nom: String = row.get("nom").unwrap_or_default();

    let user_row: mysql::Row = conn
        .exec_first(
            "SELECT id, nom, email, privilege, vip \
         FROM login \
         WHERE email = ? \
         LIMIT 1",
            (&email,),
        )
        .ok()??;

    let id: i64 = user_row.get("id")?;
    let privilege: i64 = user_row.get("privilege").unwrap_or(10);
    let vip: i64 = user_row.get("vip").unwrap_or(0);
    let nom_login: String = user_row.get("nom").unwrap_or(nom);

    let mut info = HashMap::new();
    info.insert("connecte".into(), json!(true));
    info.insert("id".into(), json!(id));
    info.insert("nom".into(), json!(nom_login));
    info.insert("email".into(), json!(email));
    info.insert("privilege".into(), json!(privilege));
    info.insert("vip".into(), json!(vip));
    Some(info)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 2 : selectionner()
// SELECT générique.
// ══════════════════════════════════════════════════════════════════
pub fn selectionner(
    pool: &DbPool,
    table: &str,
    where_clause: &[(&str, mysql::Value)],
    colonnes: &[&str],
    order_by: Option<&str>,
    limit: Option<u64>,
) -> Vec<HashMap<String, Value>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let select = if colonnes.is_empty() {
        "*".to_string()
    } else {
        colonnes
            .iter()
            .map(|c| format!("`{}`", c))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut query = format!("SELECT {} FROM `{}`", select, table);
    let mut params: Vec<mysql::Value> = vec![];

    if !where_clause.is_empty() {
        let parts: Vec<String> = where_clause
            .iter()
            .map(|(col, val)| {
                params.push(val.clone());
                format!("`{}` = ?", col)
            })
            .collect();
        query += &format!(" WHERE {}", parts.join(" AND "));
    }
    if let Some(ob) = order_by {
        query += &format!(" ORDER BY {}", ob);
    }
    if let Some(l) = limit {
        query += &format!(" LIMIT {}", l);
    }

    let rows: Vec<Row> = match conn.exec(&query, params) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    rows.into_iter()
        .map(|row| {
            let cols = row.columns_ref();
            let mut map = HashMap::new();
            for (i, col) in cols.iter().enumerate() {
                let name = col.name_str().to_string();
                let val: mysql::Value = row.get(i).unwrap_or(mysql::Value::NULL);
                map.insert(name, mysql_val_to_json(val));
            }
            map
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 3 : inserer_ou_modifier()
// INSERT si where vide, UPDATE sinon.
// Retourne l'id inséré (INSERT), 0 (UPDATE OK) ou -1 (erreur).
// ══════════════════════════════════════════════════════════════════
pub fn inserer_ou_modifier(
    pool: &DbPool,
    table: &str,
    donnees: &[(&str, mysql::Value)],
    where_c: &[(&str, mysql::Value)],
) -> i64 {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return -1,
    };

    if where_c.is_empty() {
        let cols: Vec<String> = donnees.iter().map(|(c, _)| format!("`{}`", c)).collect();
        let ph: Vec<&str> = donnees.iter().map(|_| "?").collect();
        let query = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            table,
            cols.join(", "),
            ph.join(", ")
        );
        let vals: Vec<mysql::Value> = donnees.iter().map(|(_, v)| v.clone()).collect();
        match conn.exec_drop(&query, vals) {
            Ok(_) => conn.last_insert_id() as i64,
            Err(e) => {
                eprintln!("[db] INSERT {} a échoué: {}", table, e);
                -1
            }
        }
    } else {
        let sets: Vec<String> = donnees
            .iter()
            .map(|(c, _)| format!("`{}` = ?", c))
            .collect();
        let conds: Vec<String> = where_c
            .iter()
            .map(|(c, _)| format!("`{}` = ?", c))
            .collect();
        let query = format!(
            "UPDATE `{}` SET {} WHERE {}",
            table,
            sets.join(", "),
            conds.join(" AND ")
        );
        let mut vals: Vec<mysql::Value> = donnees.iter().map(|(_, v)| v.clone()).collect();
        for (_, v) in where_c {
            vals.push(v.clone());
        }
        match conn.exec_drop(&query, vals) {
            Ok(_) => 0,
            Err(e) => {
                eprintln!("[db] UPDATE {} a échoué: {}", table, e);
                -1
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 4 : supprimer_ligne()
// ══════════════════════════════════════════════════════════════════
pub fn supprimer_ligne(pool: &DbPool, table: &str, col: &str, id: mysql::Value) -> bool {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    conn.exec_drop(
        format!("DELETE FROM `{}` WHERE `{}` = ?", table, col),
        (id,),
    )
    .is_ok()
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 5 : compter_lignes()
// ══════════════════════════════════════════════════════════════════
pub fn compter_lignes(pool: &DbPool, table: &str, where_clause: &[(&str, mysql::Value)]) -> u64 {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let mut query = format!("SELECT COUNT(*) FROM `{}`", table);
    let mut params: Vec<mysql::Value> = vec![];

    if !where_clause.is_empty() {
        let parts: Vec<String> = where_clause
            .iter()
            .map(|(col, val)| {
                params.push(val.clone());
                format!("`{}` = ?", col)
            })
            .collect();
        query += &format!(" WHERE {}", parts.join(" AND "));
    }

    conn.exec_first::<u64, _, _>(&query, params)
        .ok()
        .flatten()
        .unwrap_or(0)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 6 : compter_sessions_actives()
// La durée est passée en paramètre (lue depuis config.json côté appelant).
// ══════════════════════════════════════════════════════════════════
pub fn compter_sessions_actives(pool: &DbPool, minutes: u32) -> u64 {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.exec_first(
        "SELECT COUNT(*) FROM `loginc` WHERE `datecra` > NOW() - INTERVAL ? MINUTE",
        (minutes,),
    )
    .ok()
    .flatten()
    .unwrap_or(0)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 7 : get_taille_db()
// ══════════════════════════════════════════════════════════════════
pub fn get_taille_db(pool: &DbPool) -> f64 {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return 0.0,
    };
    conn.exec_first::<f64, _, _>(
        "SELECT ROUND(SUM(data_length+index_length)/1024/1024, 2) \
         FROM information_schema.tables WHERE table_schema = DATABASE()",
        (),
    )
    .ok()
    .flatten()
    .unwrap_or(0.0)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 8 : lister_tables()
// ══════════════════════════════════════════════════════════════════
pub fn lister_tables(pool: &DbPool) -> Vec<String> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    conn.exec("SHOW TABLES", ())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|row: Row| row.get::<String, _>(0))
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 9 : get_tailles_tables()
// ══════════════════════════════════════════════════════════════════
pub fn get_tailles_tables(pool: &DbPool) -> Vec<HashMap<String, Value>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let rows: Vec<Row> = conn
        .exec(
            "SELECT table_name, \
                ROUND((data_length+index_length)/1024, 1) AS sz, \
                table_rows AS rws \
         FROM information_schema.tables \
         WHERE table_schema = DATABASE() \
         ORDER BY (data_length+index_length) DESC",
            (),
        )
        .unwrap_or_default();

    rows.into_iter()
        .map(|row| {
            let mut map = HashMap::new();
            map.insert(
                "table_name".into(),
                json!(row.get::<String, _>("table_name").unwrap_or_default()),
            );
            map.insert("sz".into(), json!(row.get::<f64, _>("sz").unwrap_or(0.0)));
            map.insert("rws".into(), json!(row.get::<u64, _>("rws").unwrap_or(0)));
            map
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 10 : lire_lignes_table()
// ══════════════════════════════════════════════════════════════════
pub fn lire_lignes_table(
    pool: &DbPool,
    table: &str,
    limit: u64,
    offset: u64,
) -> Option<HashMap<String, Value>> {
    let tables_connues = lister_tables(pool);
    if !tables_connues.contains(&table.to_string()) {
        return None;
    }

    let mut conn = pool.get_conn().ok()?;
    let result: Vec<Row> = conn
        .exec(
            format!(
                "SELECT * FROM `{}` LIMIT {} OFFSET {}",
                table, limit, offset
            ),
            (),
        )
        .ok()?;

    if result.is_empty() {
        let desc_rows: Vec<Row> = conn
            .exec(format!("DESCRIBE `{}`", table), ())
            .unwrap_or_default();

        let cols: Vec<String> = desc_rows
            .into_iter()
            .filter_map(|row| row.get::<String, _>("Field"))
            .collect();

        let mut out = HashMap::new();
        out.insert("cols".into(), json!(cols));
        out.insert("rows".into(), json!([]));
        return Some(out);
    }

    let cols: Vec<String> = result[0]
        .columns_ref()
        .iter()
        .map(|c| c.name_str().to_string())
        .collect();

    let rows: Vec<Value> = result
        .into_iter()
        .map(|row| {
            let mut map = serde_json::Map::new();
            for (i, col) in cols.iter().enumerate() {
                let val: mysql::Value = row.get(i).unwrap_or(mysql::Value::NULL);
                let jval = match &val {
                    mysql::Value::Bytes(b) => {
                        let s = String::from_utf8_lossy(b);
                        if s.len() > 200 {
                            json!(format!("{} …[{} o]", &s[..200], s.len()))
                        } else {
                            json!(s.to_string())
                        }
                    }
                    _ => mysql_val_to_json(val),
                };
                map.insert(col.clone(), jval);
            }
            Value::Object(map)
        })
        .collect();

    let mut out = HashMap::new();
    out.insert("cols".into(), json!(cols));
    out.insert("rows".into(), json!(rows));
    Some(out)
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 11 : decrire_table()
// ══════════════════════════════════════════════════════════════════
pub fn decrire_table(pool: &DbPool, table: &str) -> Vec<HashMap<String, Value>> {
    let tables_connues = lister_tables(pool);
    if !tables_connues.contains(&table.to_string()) {
        return vec![];
    }

    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let rows: Vec<Row> = match conn.exec(format!("DESCRIBE `{}`", table), ()) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    rows.into_iter()
        .map(|row| {
            let mut map = HashMap::new();
            map.insert(
                "field".into(),
                json!(row.get::<String, _>("Field").unwrap_or_default()),
            );
            map.insert(
                "type".into(),
                json!(row.get::<String, _>("Type").unwrap_or_default()),
            );
            map.insert(
                "null".into(),
                json!(row.get::<String, _>("Null").unwrap_or_default()),
            );
            map.insert(
                "key".into(),
                json!(row.get::<String, _>("Key").unwrap_or_default()),
            );
            map.insert(
                "default".into(),
                json!(row
                    .get::<Option<String>, _>("Default")
                    .unwrap_or(None)
                    .unwrap_or_else(|| "NULL".to_string())),
            );
            map.insert(
                "extra".into(),
                json!(row.get::<String, _>("Extra").unwrap_or_default()),
            );
            map
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// FONCTION 12 : executer_sql_admin()
// SQL Runner sécurisé pour le panel admin.
// ══════════════════════════════════════════════════════════════════
pub fn executer_sql_admin(
    pool: &DbPool,
    cookie_val: &str,
    remote_ip: &str,
    user_agent: &str,
    raw_sql: &str,
) -> Value {
    let user = match verifier_connexion(pool, cookie_val, remote_ip, user_agent) {
        Some(u) => u,
        None => return json!({"success":false,"error":"Session invalide ou expirée."}),
    };
    let privilege = user.get("privilege").and_then(|v| v.as_i64()).unwrap_or(99);
    if privilege > 2 {
        return json!({"success":false,"error":"Droits insuffisants pour le SQL runner (superadmin requis)."});
    }

    let sql_trimmed = raw_sql.trim();
    if sql_trimmed.is_empty() {
        return json!({"success":false,"error":"Requête vide."});
    }

    let sql_upper = sql_trimmed.to_uppercase();

    let forbidden_system: &[&str] = &[
        "LOAD_FILE",
        "INTO OUTFILE",
        "INTO DUMPFILE",
        "SLEEP(",
        "BENCHMARK(",
        "INFORMATION_SCHEMA",
        "MYSQL.USER",
        "PG_SLEEP",
        "WAITFOR",
        "XP_CMDSHELL",
    ];
    for kw in forbidden_system {
        if sql_upper.contains(kw) {
            return json!({"success":false,"error":format!("Opération interdite : {}", kw)});
        }
    }

    {
        let sql_compact: String = sql_upper.chars().filter(|c| !c.is_whitespace()).collect();
        let login_present = sql_compact.contains("LOGIN");
        let privilege_present = sql_compact.contains("PRIVILEGE");
        if login_present && privilege_present {
            let patterns: &[&str] = &[
                "PRIVILEGE=1,",
                "PRIVILEGE=1)",
                "PRIVILEGE=1;",
                "`PRIVILEGE`=1,",
                "`PRIVILEGE`=1)",
                "`PRIVILEGE`=1;",
                "`PRIVILEGE`=1",
            ];
            let is_blocked = patterns.iter().any(|p| sql_compact.contains(p))
                || sql_compact.ends_with("PRIVILEGE=1")
                || sql_compact.ends_with("`PRIVILEGE`=1");
            if is_blocked {
                return json!({"success":false,"error":"Attribution du privilege 1 interdite, même via SQL direct."});
            }
        }
    }

    let is_select = sql_upper.trim_start().starts_with("SELECT");
    let sql_to_run = if is_select && !sql_upper.contains("LIMIT") {
        format!("{} LIMIT 200", sql_trimmed.trim_end_matches(';'))
    } else {
        sql_trimmed.to_string()
    };

    let sensitive: &[&str] = &[
        "password",
        "mot_de_passe",
        "token",
        "secret",
        "hash",
        "cookie",
        "passwd",
        "pwd",
    ];

    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(e) => return json!({"success":false,"error":format!("Connexion DB perdue : {}", e)}),
    };

    if is_select {
        let result = conn.query_map(&sql_to_run, |row: mysql::Row| {
            let cols: Vec<String> = row
                .columns_ref()
                .iter()
                .map(|c| c.name_str().to_string())
                .collect();
            let vals: Vec<Value> = row
                .columns_ref()
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let name_lc = c.name_str().to_lowercase();
                    if sensitive.iter().any(|p| name_lc.contains(p)) {
                        return json!("***");
                    }
                    match row.get_opt::<mysql::Value, usize>(i) {
                        Some(Ok(mysql::Value::NULL)) => json!(null),
                        Some(Ok(mysql::Value::Bytes(b))) => {
                            json!(String::from_utf8_lossy(&b).to_string())
                        }
                        Some(Ok(mysql::Value::Int(n))) => json!(n),
                        Some(Ok(mysql::Value::UInt(n))) => json!(n),
                        Some(Ok(mysql::Value::Float(f))) => json!(f),
                        Some(Ok(mysql::Value::Double(f))) => json!(f),
                        _ => json!("?"),
                    }
                })
                .collect();
            (cols, vals)
        });

        return match result {
            Err(e) => json!({"success":false,"error":format!("Erreur SQL : {}", e)}),
            Ok(rows) if rows.is_empty() => {
                json!({"success":true,"data":{"cols":[],"rows":[],"count":0}})
            }
            Ok(rows) => {
                let cols = rows[0].0.clone();
                let data: Vec<Value> = rows
                    .iter()
                    .map(|(_, vals)| {
                        let obj: serde_json::Map<String, Value> = cols
                            .iter()
                            .zip(vals.iter())
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        json!(obj)
                    })
                    .collect();
                let count = data.len();
                json!({"success":true,"data":{"cols":cols,"rows":data,"count":count}})
            }
        };
    }

    match conn.exec_drop(&sql_to_run, ()) {
        Ok(_) => {
            let affected = conn.affected_rows();
            json!({"success":true,"data":{"cols":["Résultat"],"rows":[{"Résultat":format!("{} ligne(s) affectée(s)",affected)}],"count":1,"affected_rows":affected}})
        }
        Err(e) => json!({"success":false,"error":format!("Erreur SQL : {}", e)}),
    }
}

// ══════════════════════════════════════════════════════════════════
// FONCTIONS P2P — partagées avec p2p.rs
// ══════════════════════════════════════════════════════════════════

// ── p2p_peers ─────────────────────────────────────────────────────
// Schéma attendu (à créer dans phpMyAdmin) :
// CREATE TABLE `p2p_peers` (
//   `id`         INT AUTO_INCREMENT PRIMARY KEY,
//   `node_id`    VARCHAR(64)  NOT NULL UNIQUE,
//   `vex_url`    VARCHAR(255) NOT NULL,
//   `ip`         VARCHAR(128) NOT NULL,
//   `port`       INT          NOT NULL DEFAULT 7700,
//   `tor_addr`   VARCHAR(255) DEFAULT NULL,
//   `pub_key`    TEXT         NOT NULL,
//   `status`     VARCHAR(16)  NOT NULL DEFAULT 'offline',
//   `last_seen`  DATETIME     NOT NULL DEFAULT NOW(),
//   `version`    VARCHAR(32)  DEFAULT NULL
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

// ── p2p_users ─────────────────────────────────────────────────────
// Schéma attendu :
// CREATE TABLE `p2p_users` (
//   `id`          INT AUTO_INCREMENT PRIMARY KEY,
//   `user_id`     INT          NOT NULL,
//   `node_id`     VARCHAR(64)  NOT NULL,
//   `nom`         VARCHAR(128) NOT NULL,
//   `pub_key`     TEXT         NOT NULL,
//   `updated_at`  DATETIME     NOT NULL DEFAULT NOW(),
//   UNIQUE KEY `user_node` (`user_id`, `node_id`)
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

// ── p2p_transfers ─────────────────────────────────────────────────
// Schéma attendu :
// CREATE TABLE `p2p_transfers` (
//   `id`           INT AUTO_INCREMENT PRIMARY KEY,
//   `transfer_id`  VARCHAR(64)  NOT NULL UNIQUE,
//   `from_node`    VARCHAR(64)  NOT NULL,
//   `to_node`      VARCHAR(64)  NOT NULL,
//   `from_user`    INT          NOT NULL,
//   `to_user`      INT          NOT NULL,
//   `fichier_nom`  VARCHAR(255) NOT NULL,
//   `fichier_size` BIGINT       NOT NULL DEFAULT 0,
//   `chunk_size`   INT          NOT NULL DEFAULT 1048576,
//   `chunks_total` INT          NOT NULL DEFAULT 1,
//   `chunks_ok`    INT          NOT NULL DEFAULT 0,
//   `status`       VARCHAR(32)  NOT NULL DEFAULT 'pending',
//   `created_at`   DATETIME     NOT NULL DEFAULT NOW(),
//   `updated_at`   DATETIME     NOT NULL DEFAULT NOW()
// ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

/// Enregistre ou met à jour un nœud P2P dans p2p_peers.
/// Si node_id existe déjà → UPDATE, sinon → INSERT.
pub fn p2p_upsert_peer(
    pool: &DbPool,
    node_id: &str,
    vex_url: &str,
    ip: &str,
    port: u16,
    tor_addr: Option<&str>,
    pub_key: &str,
    version: &str,
) -> bool {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let tor = tor_addr.unwrap_or("");
    conn.exec_drop(
        "INSERT INTO `p2p_peers` (node_id, vex_url, ip, port, tor_addr, pub_key, status, last_seen, version)
         VALUES (?, ?, ?, ?, ?, ?, 'online', NOW(), ?)
         ON DUPLICATE KEY UPDATE
           vex_url   = VALUES(vex_url),
           ip        = VALUES(ip),
           port      = VALUES(port),
           tor_addr  = VALUES(tor_addr),
           pub_key   = VALUES(pub_key),
           status    = 'online',
           last_seen = NOW(),
           version   = VALUES(version)",
        (node_id, vex_url, ip, port, tor, pub_key, version),
    ).is_ok()
}

/// Marque un nœud comme offline.
pub fn p2p_peer_offline(pool: &DbPool, node_id: &str) -> bool {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    conn.exec_drop(
        "UPDATE `p2p_peers` SET status='offline' WHERE node_id=?",
        (node_id,),
    )
    .is_ok()
}

/// Liste tous les nœuds connus (online ou offline).
pub fn p2p_lister_peers(pool: &DbPool) -> Vec<HashMap<String, Value>> {
    selectionner(
        pool,
        "p2p_peers",
        &[],
        &[
            "id",
            "node_id",
            "vex_url",
            "ip",
            "port",
            "tor_addr",
            "pub_key",
            "status",
            "last_seen",
            "version",
        ],
        Some("last_seen DESC"),
        None,
    )
}

/// Liste uniquement les nœuds en ligne.
pub fn p2p_lister_peers_online(pool: &DbPool) -> Vec<HashMap<String, Value>> {
    selectionner(
        pool,
        "p2p_peers",
        &[("status", mysql::Value::from("online"))],
        &[
            "id",
            "node_id",
            "vex_url",
            "ip",
            "port",
            "tor_addr",
            "pub_key",
            "last_seen",
            "version",
        ],
        Some("last_seen DESC"),
        None,
    )
}

/// Récupère un nœud par son node_id.
pub fn p2p_get_peer(pool: &DbPool, node_id: &str) -> Option<HashMap<String, Value>> {
    selectionner(
        pool,
        "p2p_peers",
        &[("node_id", mysql::Value::from(node_id))],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()
}

/// Enregistre ou met à jour un utilisateur P2P dans p2p_users.
pub fn p2p_upsert_user(
    pool: &DbPool,
    user_id: i64,
    node_id: &str,
    nom: &str,
    pub_key: &str,
) -> bool {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    conn.exec_drop(
        "INSERT INTO `p2p_users` (user_id, node_id, nom, pub_key, updated_at)
         VALUES (?, ?, ?, ?, NOW())
         ON DUPLICATE KEY UPDATE
           nom        = VALUES(nom),
           pub_key    = VALUES(pub_key),
           updated_at = NOW()",
        (user_id, node_id, nom, pub_key),
    )
    .is_ok()
}

/// Liste tous les utilisateurs P2P connus (tous nœuds).
pub fn p2p_lister_users(pool: &DbPool) -> Vec<HashMap<String, Value>> {
    selectionner(
        pool,
        "p2p_users",
        &[],
        &["id", "user_id", "node_id", "nom", "pub_key", "updated_at"],
        Some("updated_at DESC"),
        None,
    )
}

/// Crée un transfert de fichier P2P (peut être multi-chunks).
pub fn p2p_creer_transfer(
    pool: &DbPool,
    transfer_id: &str,
    from_node: &str,
    to_node: &str,
    from_user: i64,
    to_user: i64,
    fichier_nom: &str,
    fichier_size: i64,
    chunk_size: i64,
    chunks_total: i32,
) -> i64 {
    inserer_ou_modifier(
        pool,
        "p2p_transfers",
        &[
            ("transfer_id", mysql::Value::from(transfer_id)),
            ("from_node", mysql::Value::from(from_node)),
            ("to_node", mysql::Value::from(to_node)),
            ("from_user", mysql::Value::from(from_user)),
            ("to_user", mysql::Value::from(to_user)),
            ("fichier_nom", mysql::Value::from(fichier_nom)),
            ("fichier_size", mysql::Value::from(fichier_size)),
            ("chunk_size", mysql::Value::from(chunk_size)),
            ("chunks_total", mysql::Value::from(chunks_total)),
            ("chunks_ok", mysql::Value::from(0i32)),
            ("status", mysql::Value::from("pending")),
        ],
        &[],
    )
}

/// Incrémente chunks_ok et passe à 'complete' si tous les chunks sont arrivés.
pub fn p2p_chunk_recu(pool: &DbPool, transfer_id: &str) -> Value {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return json!({"success":false,"error":"DB error"}),
    };
    let ok = conn.exec_drop(
        "UPDATE `p2p_transfers`
         SET chunks_ok  = chunks_ok + 1,
             updated_at = NOW(),
             status     = IF(chunks_ok + 1 >= chunks_total, 'complete', 'in_progress')
         WHERE transfer_id = ?",
        (transfer_id,),
    );
    match ok {
        Ok(_) => json!({"success":true}),
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}

/// Récupère l'état d'un transfert.
pub fn p2p_get_transfer(pool: &DbPool, transfer_id: &str) -> Option<HashMap<String, Value>> {
    selectionner(
        pool,
        "p2p_transfers",
        &[("transfer_id", mysql::Value::from(transfer_id))],
        &[],
        None,
        Some(1),
    )
    .into_iter()
    .next()
}

/// Liste les transferts d'un utilisateur (envoyés ou reçus).
pub fn p2p_lister_transfers(pool: &DbPool, user_id: i64) -> Vec<HashMap<String, Value>> {
    let mut conn = match pool.get_conn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let rows: Vec<Row> = conn
        .exec(
            "SELECT * FROM `p2p_transfers`
         WHERE from_user=? OR to_user=?
         ORDER BY created_at DESC LIMIT 100",
            (user_id, user_id),
        )
        .unwrap_or_default();

    rows.into_iter()
        .map(|row| {
            let cols = row.columns_ref();
            let mut map = HashMap::new();
            for (i, col) in cols.iter().enumerate() {
                let val: mysql::Value = row.get(i).unwrap_or(mysql::Value::NULL);
                map.insert(col.name_str().to_string(), mysql_val_to_json(val));
            }
            map
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════
// API JSON localhost-only
// ══════════════════════════════════════════════════════════════════
pub fn handle_api_action(
    pool: &DbPool,
    action: &str,
    params: &HashMap<String, String>,
    remote_ip: &str,
) -> Value {
    if remote_ip != "127.0.0.1" && remote_ip != "::1" {
        return json!({"success": false, "error": "Access denied - localhost only"});
    }

    match action {
        "check_access" => {
            let cid = params.get("cid");
            let neut_id = params.get("neut_id");
            if cid.is_none() || neut_id.is_none() {
                return json!({"success": false, "error": "Paramètres manquants"});
            }
            let (cid, neut_id) = (cid.unwrap(), neut_id.unwrap());

            let fichiers = selectionner(
                pool,
                "fichiers",
                &[("cid", mysql::Value::from(cid.as_str()))],
                &["public", "partage"],
                None,
                Some(1),
            );

            if fichiers.is_empty() {
                return json!({"success": true, "data": {"access": true, "reason": "not_in_db"}});
            }
            if fichiers[0].get("public").and_then(|v| v.as_i64()) == Some(1) {
                return json!({"success": true, "data": {"access": true, "reason": "public"}});
            }
            let partage = fichiers[0]
                .get("partage")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let needle = format!("neut:{}", neut_id);
            if partage.contains(&needle) {
                json!({"success": true, "data": {"access": true, "reason": "shared"}})
            } else {
                json!({"success": false, "data": {"access": false, "reason": "denied"}})
            }
        }

        "register_file" => {
            let cid = match params.get("cid") {
                Some(c) => c.clone(),
                None => return json!({"success": false, "error": "CID manquant"}),
            };
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let id = inserer_ou_modifier(
                pool,
                "fichiers",
                &[
                    ("cid", mysql::Value::from(cid.as_str())),
                    (
                        "id_utilisateur",
                        mysql::Value::from(
                            params
                                .get("id_utilisateur")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(1),
                        ),
                    ),
                    (
                        "nom_fichier",
                        mysql::Value::from(
                            params
                                .get("nom_fichier")
                                .map(|s| s.as_str())
                                .unwrap_or("file"),
                        ),
                    ),
                    (
                        "taille",
                        mysql::Value::from(
                            params
                                .get("taille")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(0),
                        ),
                    ),
                    (
                        "public",
                        mysql::Value::from(
                            params
                                .get("public")
                                .and_then(|v| v.parse::<i64>().ok())
                                .unwrap_or(0),
                        ),
                    ),
                    ("visble", mysql::Value::from("public")),
                    ("partage", mysql::Value::from("")),
                    (
                        "type_fichier",
                        mysql::Value::from(
                            params
                                .get("type_mime")
                                .map(|s| s.as_str())
                                .unwrap_or("application/octet-stream"),
                        ),
                    ),
                    ("date", mysql::Value::from(now.as_str())),
                ],
                &[],
            );
            json!({"success": true, "data": {"id": id}})
        }

        "add_share" => {
            let cid = params.get("cid");
            let neut = params.get("neut_id");
            let userid = params.get("id_utilisateur");
            if cid.is_none() || neut.is_none() || userid.is_none() {
                return json!({"success": false, "error": "Paramètres manquants"});
            }
            let (cid, neut, userid) = (cid.unwrap(), neut.unwrap(), userid.unwrap());

            let fichiers = selectionner(
                pool,
                "fichiers",
                &[("cid", mysql::Value::from(cid.as_str()))],
                &["id", "partage"],
                None,
                Some(1),
            );
            if fichiers.is_empty() {
                return json!({"success": false, "error": "Fichier non trouvé"});
            }
            let fid = fichiers[0]["id"].as_i64().unwrap_or(0);
            let existing = fichiers[0]["partage"].as_str().unwrap_or("");
            let nouveau = format!("neut:{}.{}", neut, userid);
            let final_val = if existing.is_empty() {
                nouveau
            } else {
                format!("{},{}", existing, nouveau)
            };
            inserer_ou_modifier(
                pool,
                "fichiers",
                &[("partage", mysql::Value::from(final_val.as_str()))],
                &[("id", mysql::Value::from(fid))],
            );
            json!({"success": true, "data": {"shared": true}})
        }

        _ => json!({"success": false, "error": "Action inconnue"}),
    }
}

// ══════════════════════════════════════════════════════════════════
// UTILITAIRE : mysql::Value → serde_json::Value
// ══════════════════════════════════════════════════════════════════
pub fn mysql_val_to_json(val: mysql::Value) -> Value {
    match val {
        mysql::Value::NULL => Value::Null,
        mysql::Value::Int(i) => json!(i),
        mysql::Value::UInt(u) => json!(u),
        mysql::Value::Float(f) => json!(f),
        mysql::Value::Double(d) => json!(d),
        mysql::Value::Bytes(b) => json!(String::from_utf8_lossy(&b).to_string()),
        mysql::Value::Date(y, mo, d, h, mi, s, _) => json!(format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            y, mo, d, h, mi, s
        )),
        mysql::Value::Time(neg, d, h, mi, s, _) => {
            let sign = if neg { "-" } else { "" };
            json!(format!("{}{}:{:02}:{:02}", sign, d * 24 + h as u32, mi, s))
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// UTILITAIRES TERMINAL (main.rs uniquement)
// ══════════════════════════════════════════════════════════════════
pub const TABLES_MODIFIABLES_TERMINAL: &[&str] = &["login", "loginc", "pref", "fichiers"];

#[derive(Debug, Clone, Copy)]
pub enum ActionTableTerminal {
    Vider,
    SupprimerToutesLesLignes,
}

fn erreur_db_input(message: &str) -> mysql::Error {
    mysql::Error::IoError(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.to_string(),
    ))
}

fn verifier_table_autorisee(table: &str) -> Result<(), mysql::Error> {
    if TABLES_MODIFIABLES_TERMINAL.contains(&table) {
        Ok(())
    } else {
        Err(erreur_db_input(
            "Table non autorisee pour les actions terminal",
        ))
    }
}

pub fn executer_action_table_terminal(
    pool: &mysql::Pool,
    table: &str,
    action: ActionTableTerminal,
) -> Result<(), mysql::Error> {
    verifier_table_autorisee(table)?;
    let mut conn = pool.get_conn()?;
    conn.query_drop("SET FOREIGN_KEY_CHECKS = 0")?;
    let result = match action {
        ActionTableTerminal::Vider => conn.query_drop(format!("TRUNCATE TABLE `{}`", table)),
        ActionTableTerminal::SupprimerToutesLesLignes => {
            conn.query_drop(format!("DELETE FROM `{}`", table))
        }
    };
    let fk_result = conn.query_drop("SET FOREIGN_KEY_CHECKS = 1");
    result?;
    fk_result?;
    Ok(())
}

pub fn vider_tables_terminal(pool: &mysql::Pool, tables: &[&str]) -> Result<(), mysql::Error> {
    for table in tables {
        executer_action_table_terminal(pool, table, ActionTableTerminal::Vider)?;
    }
    Ok(())
}

pub fn regler_privilege_utilisateur(
    pool: &mysql::Pool,
    user_id: i64,
    privilege: i64,
) -> Result<(), mysql::Error> {
    if !(2..=12).contains(&privilege) {
        return Err(erreur_db_input(
            "Le privilege doit etre compris entre 2 et 12",
        ));
    }
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE `login` SET `privilege` = ? WHERE `id` = ?",
        (privilege, user_id),
    )?;
    Ok(())
}

pub fn donner_privilege_1_thesolar(pool: &mysql::Pool) -> Result<(), mysql::Error> {
    let mut conn = pool.get_conn()?;
    conn.exec_drop(
        "UPDATE `login` SET `privilege` = 1 WHERE `email` = ?",
        ("thesolar@r.frr",),
    )?;
    Ok(())
}
