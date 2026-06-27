// ══════════════════════════════════════════════════════════════════
// main.rs — VEX server entry point
// ══════════════════════════════════════════════════════════════════
mod db_init;
mod access_control;
mod appeldb;
mod c;
mod config_loader;
mod function;
mod utils;

mod p2p {
    pub mod p2p;
}
mod admin {
    pub mod admin;
}
mod login {
    pub mod account;
    pub mod autologin;
    pub mod dashboard;
    pub mod first_setup;
    pub mod login;
    pub mod logout;
}
mod fchier {
    pub mod fchier;
}

use crate::p2p::p2p::{
    handle_request, lancer_sync_periodique, sync_avec_bootstrap, NodeState, P2pConfig,
};
use appeldb::{
    creer_pool, executer_action_table_terminal, regler_privilege_utilisateur, ActionTableTerminal,
    TABLES_MODIFIABLES_TERMINAL,
};
use config_loader::{load_config, load_db_config};
use std::env;
use std::sync::{Arc, RwLock};
use tiny_http::{Response, Server};

const CONFIG_PATH: &str = "config.json";
const DEFAULT_PORT: u16 = 8080;

fn main() {
    let args: Vec<String> = env::args().collect();
    let config = load_config(CONFIG_PATH);
    let db_config = load_db_config(CONFIG_PATH);

    if let Err(e) = db_init::init_db(&db_config) {
        eprintln!("[main] init_db échoué: {}", e);
        std::process::exit(1);
    }

    let pool = match creer_pool(&db_config) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[main] MySQL : {}", e);
            std::process::exit(1);
        }
    };
    let _ = appeldb::donner_privilege_1_thesolar(&pool);

    if let Some(exit_code) = handle_terminal_db_commands(&args, &pool) {
        std::process::exit(exit_code);
    }

    if args.contains(&"--reset-loginc".to_string()) {
        match executer_action_table_terminal(&pool, "loginc", ActionTableTerminal::Vider) {
            Ok(()) => {
                println!("reset_table(loginc) OK");
                return;
            }
            Err(e) => {
                eprintln!("reset_table(loginc) ERREUR: {}", e);
                std::process::exit(1);
            }
        }
    }

    // ── Port d'écoute HTTP ────────────────────────────────────────
    let port = config
        .extra
        .get("server")
        .and_then(|s| s.get("port"))
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_PORT as u64) as u16;

    // ── Init P2P ─────────────────────────────────────────────────
    // Lit la section "p2p" de config.json.
    // Si absente, des valeurs par défaut sont utilisées.
    let vex_url = config
        .extra
        .get("server")
        .and_then(|s| s.get("public_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8080")
        .to_string();

    let p2p_cfg = P2pConfig::from_vex_config(&config);
    let node_state = Arc::new(RwLock::new(NodeState::init(&vex_url, p2p_cfg)));

    {
        let ns = node_state.read().unwrap();
        eprintln!("[P2P] node_id = {}", ns.node_id);
        eprintln!("[P2P] pub_key = {}", ns.pub_key_b64());
        eprintln!("[P2P] bootstrap = {}", ns.config.bootstrap_url);
    }

    // Enregistrement immédiat auprès du bootstrap + sync initiale
    {
        let ns = node_state.read().unwrap();
        let pool_clone = pool.clone();
        sync_avec_bootstrap(&pool_clone, &ns);
    }

    // Sync périodique en arrière-plan
    lancer_sync_periodique(pool.clone(), Arc::clone(&node_state));

    // ── Serveur HTTP ──────────────────────────────────────────────
    let server = match Server::http(format!("0.0.0.0:{}", port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[main] Serveur : {}", e);
            std::process::exit(1);
        }
    };

    eprintln!("[VEX] http://0.0.0.0:{}", port);

    for mut request in server.incoming_requests() {
        let url = request.url().to_string();
        let method = request.method().to_string();

        let remote_full = request
            .remote_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".into());
        let remote = utils::strip_port(&remote_full);

        let path = url.split('?').next().unwrap_or(&url).to_string();

        if config.app.debug_mode {
            eprintln!("[{}] {} {}", remote, method, path);
        }

        match path.as_str() {
            "/" | "/login" | "/login/" | "/login/login" | "/login/login.php" => {
                login::login::handle_request(request, &pool, &config, &remote);
            }

            "/login/first_setup" => {
                login::first_setup::handle_request(request, &pool, &config, &remote);
            }

            "/api/login/config" => {
                login::login::handle_request(request, &pool, &config, &remote);
            }

            "/login/account" | "/login/account/" => {
                login::account::handle_request(request, &pool, &config, &remote);
            }

            p if p.starts_with("/api/account") => {
                login::account::handle_request(request, &pool, &config, &remote);
            }

            "/logout" | "/logout/" | "/login/logout" | "/login/logout/" => {
                login::logout::handle_request(request, &pool, &remote);
            }

            p if p == "/autologin"
                || p == "/autologin/"
                || p.starts_with("/autologin/")
                || p == "/login/autologin"
                || p == "/login/autologin/" =>
            {
                login::autologin::handle_request(request, &pool, &config, &remote);
            }

            "/dashboard" | "/dashboard/" | "/login/dashboard" | "/login/dashboard/" => {
                login::dashboard::handle_request(request, &pool, &config, &remote);
            }

            p if p.starts_with("/admin") || p.starts_with("/api/admin") => {
                admin::admin::handle_request(request, &pool, &config, CONFIG_PATH, &remote_full);
            }

            p if p.starts_with("/fchier") || p.starts_with("/api/fchier") => {
                let resp = fchier::fchier::handle(&pool, &mut request);
                let _ = request.respond(resp);
            }

            // ── Routes P2P publiques (inter-nœuds) ───────────────
            // /p2p/* et /neut/* sont accessibles sans authentification.
            // La sécurité repose sur la vérification de signature Ed25519
            // à l'intérieur de chaque handler dans p2p.rs.
            p if p.starts_with("/p2p/") || p.starts_with("/neut/") => {
                handle_request(request, &pool, &node_state, &config);
            }

            "/api/db" => {
                let params = utils::parse_query(&url);
                let action = params.get("action").cloned().unwrap_or_default();
                let resp = appeldb::handle_api_action(&pool, &action, &params, &remote);
                respond_json(request, resp);
            }

            p if is_static(p) => {
                serve_static(request, p);
            }

            "/health" => {
                let _ = request.respond(Response::from_string("ok"));
            }

            _ => {
                let _ =
                    request.respond(Response::from_string("404 Not Found").with_status_code(404));
            }
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Commandes terminal DB
// ══════════════════════════════════════════════════════════════════
fn handle_terminal_db_commands(args: &[String], pool: &appeldb::DbPool) -> Option<i32> {
    if args.iter().any(|a| a == "--help-db") {
        print_db_help();
        return Some(0);
    }

    if let Some(pos) = args.iter().position(|a| a == "--table-action") {
        let table = match args.get(pos + 1) {
            Some(v) => v.as_str(),
            None => {
                eprintln!("Usage: --table-action <table> <vider|supprimer-lignes>");
                return Some(1);
            }
        };
        let action_raw = match args.get(pos + 2) {
            Some(v) => v.as_str(),
            None => {
                eprintln!("Usage: --table-action <table> <vider|supprimer-lignes>");
                return Some(1);
            }
        };
        let action = match action_raw {
            "vider" => ActionTableTerminal::Vider,
            "supprimer-lignes" => ActionTableTerminal::SupprimerToutesLesLignes,
            _ => {
                eprintln!(
                    "Action inconnue: {}. Utilise 'vider' ou 'supprimer-lignes'.",
                    action_raw
                );
                return Some(1);
            }
        };
        match executer_action_table_terminal(pool, table, action) {
            Ok(()) => {
                println!("Action '{}' executee sur la table '{}'.", action_raw, table);
                return Some(0);
            }
            Err(e) => {
                eprintln!("Erreur table '{}': {}", table, e);
                return Some(1);
            }
        }
    }

    if let Some(pos) = args.iter().position(|a| a == "--set-privilege") {
        let user_id = match args.get(pos + 1).and_then(|v| v.parse::<i64>().ok()) {
            Some(v) => v,
            None => {
                eprintln!("Usage: --set-privilege <user_id> <privilege>");
                return Some(1);
            }
        };
        let privilege = match args.get(pos + 2).and_then(|v| v.parse::<i64>().ok()) {
            Some(v) => v,
            None => {
                eprintln!("Usage: --set-privilege <user_id> <privilege>");
                return Some(1);
            }
        };
        match regler_privilege_utilisateur(pool, user_id, privilege) {
            Ok(()) => {
                println!(
                    "Privilege de l'utilisateur {} regle a {}.",
                    user_id, privilege
                );
                return Some(0);
            }
            Err(e) => {
                eprintln!("Erreur set-privilege: {}", e);
                return Some(1);
            }
        }
    }

    None
}

fn print_db_help() {
    println!("Commandes DB terminal disponibles:");
    println!("  cargo run -- --table-action <table> <vider|supprimer-lignes>");
    println!("  cargo run -- --set-privilege <user_id> <privilege>");
    println!(
        "Tables autorisees: {}",
        TABLES_MODIFIABLES_TERMINAL.join(", ")
    );
    println!("Privilege autorise: entre 2 et 12");
}

// ══════════════════════════════════════════════════════════════════
// Fichiers statiques
// ══════════════════════════════════════════════════════════════════
fn is_static(path: &str) -> bool {
    path.starts_with("/static/")
        || path.ends_with(".png")
        || path.ends_with(".ico")
        || path.ends_with(".js")
        || path.ends_with(".css")
        || path.ends_with(".svg")
        || path.ends_with(".woff2")
}

fn serve_static(request: tiny_http::Request, path: &str) {
    if path.contains("..") {
        let _ = request.respond(Response::from_string("403").with_status_code(403));
        return;
    }
    let file_path = format!(".{}", path);
    match std::fs::read(&file_path) {
        Ok(data) => {
            let _ = request.respond(Response::from_data(data).with_header(
                tiny_http::Header::from_bytes("Content-Type", guess_mime(path)).unwrap(),
            ));
        }
        Err(_) => {
            let _ = request.respond(Response::from_string("404").with_status_code(404));
        }
    }
}

fn guess_mime(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

fn respond_json(request: tiny_http::Request, body: serde_json::Value) {
    let _ = request.respond(Response::from_string(body.to_string()).with_header(
        tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    ));
}
