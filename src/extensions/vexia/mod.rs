// ══════════════════════════════════════════════════════════════════
// extensions/vexia — Assistant IA (VexIA)
//
// Intègre un assistant conversationnel (Claude via l'API Anthropic)
// dans VEX. Chaque instance VEX fournit SA PROPRE clé API — configurée
// dans config.json (extensions.extension_params.vexia.params.api_key),
// jamais celle d'un compte personnel navigateur automatisé. Facturée
// normalement par le fournisseur, comme n'importe quel usage API.
//
// Servi sur /ext/vexia (panneau autonome), API sur /api/ext/vexia/...
// Le widget flottant (static/extensions/vexia/vexia-widget.js) permet
// de l'appeler depuis d'autres pages (fchier, sitec) via la même API.
// Le privilege et le plan sont deja verifies par access_control.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::DbPool;
use crate::c::SessionInfo;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Cursor;
use tiny_http::{Header, Request, Response};

const CONFIG_PATH: &str = "config.json";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_MAX_TOKENS: u64 = 1024;
const MAX_MESSAGE_LEN: usize = 32_000;
const MAX_HISTORY: usize = 20;

pub fn handle(
    pool: &DbPool,
    session: &SessionInfo,
    req: &mut Request,
) -> Response<Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let chemin = url.split('?').next().unwrap_or(&url).to_string();

    if let Some(sous) = chemin.strip_prefix("/api/ext/vexia") {
        let sous = sous.trim_end_matches('/');
        let reponse = match sous {
            "/status" | "" => statut(),
            "/chat" => chat(req),
            _ => json!({"success": false, "error": "Route VexIA inconnue."}),
        };
        return json_response(reponse);
    }

    // ── Page ─────────────────────────────────────────────────────
    match std::fs::read_to_string("static/extensions/vexia/index.html") {
        Ok(html) => {
            let prefs = crate::function::get_user_preferences(pool, session.user_id);
            let theme = if prefs.teme == 1 { "dark" } else { "light" };
            let nav = crate::access_control::nav_extension(pool, session, req, "vexia");
            let html = html
                .replace("__NAV_HTML__", &nav)
                .replace("__THEME__", theme)
                .replace("__LANG__", &prefs.langue)
                .replace("__USER_NOM__", &echapper(&session.user_nom))
                .replace("__USER_EMAIL__", &echapper(&session.user_email))
                .replace("__USER_ID__", &session.user_id.to_string());
            Response::from_string(html).with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            )
        }
        Err(e) => Response::from_string(format!(
            "<h1>VexIA</h1><p>Page introuvable : static/extensions/vexia/index.html ({})</p>",
            e
        ))
        .with_status_code(500)
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()),
    }
}

// ══════════════════════════════════════════════════════════════════
// Config
// ══════════════════════════════════════════════════════════════════

struct VexiaConfig {
    api_key: String,
    model: String,
    max_tokens: u64,
    system_prompt: String,
}

fn charger_config() -> VexiaConfig {
    let cfg = crate::config_loader::load_config(CONFIG_PATH);
    let params = cfg
        .extensions
        .extension_params
        .get("vexia")
        .and_then(|e| e.get("params"))
        .cloned()
        .unwrap_or(json!({}));

    VexiaConfig {
        api_key: params
            .get("api_key")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        model: params
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_MODEL)
            .to_string(),
        max_tokens: params
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_MAX_TOKENS),
        system_prompt: params
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Tu es VexIA, l'assistant integre a VEX (auto-heberge). Reponds de facon concise et utile, en francais sauf si on te parle dans une autre langue.")
            .to_string(),
    }
}

/// Indique au front si une cle API est configuree, sans jamais l'exposer.
fn statut() -> Value {
    let cfg = charger_config();
    json!({"success": true, "data": {"configure": !cfg.api_key.is_empty(), "model": cfg.model}})
}

// ══════════════════════════════════════════════════════════════════
// Chat — proxy vers l'API Anthropic (Messages API)
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct ChatMsg {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatBody {
    message: String,
    #[serde(default)]
    history: Vec<ChatMsg>,
}

fn chat(req: &mut Request) -> Value {
    let cfg = charger_config();
    if cfg.api_key.is_empty() {
        return json!({"success": false,
            "error": "VexIA n'est pas configure : ajoutez votre cle API Anthropic dans config.json (extensions.extension_params.vexia.params.api_key)."});
    }

    let mut brut = String::new();
    if std::io::Read::read_to_string(req.as_reader(), &mut brut).is_err() {
        return json!({"success": false, "error": "Corps de requete illisible."});
    }
    let corps: ChatBody = match serde_json::from_str(&brut) {
        Ok(c) => c,
        Err(_) => return json!({"success": false, "error": "Requete invalide (JSON attendu : {message, history?})."}),
    };

    let message = corps.message.trim();
    if message.is_empty() {
        return json!({"success": false, "error": "Message vide."});
    }
    if message.len() > MAX_MESSAGE_LEN {
        return json!({"success": false, "error": "Message trop long."});
    }

    // Historique borné — anti-DoS (une conversation ne doit pas pouvoir
    // faire grossir indéfiniment le payload envoyé à l'API à chaque tour).
    let mut messages: Vec<Value> = corps
        .history
        .iter()
        .rev()
        .take(MAX_HISTORY)
        .rev()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();
    messages.push(json!({"role": "user", "content": message}));

    let payload = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "system": cfg.system_prompt,
        "messages": messages,
    });

    let resp = ureq::post(ANTHROPIC_API_URL)
        .set("x-api-key", &cfg.api_key)
        .set("anthropic-version", ANTHROPIC_VERSION)
        .set("content-type", "application/json")
        .timeout(std::time::Duration::from_secs(60))
        .send_json(payload);

    match resp {
        Ok(r) => {
            let body: Value = r.into_json().unwrap_or(json!({}));
            let texte = body
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.iter().find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")))
                .and_then(|b| b.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if texte.is_empty() {
                json!({"success": false, "error": "Reponse vide du modele."})
            } else {
                json!({"success": true, "reply": texte})
            }
        }
        Err(ureq::Error::Status(code, r)) => {
            let detail = r
                .into_json::<Value>()
                .ok()
                .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| format!("HTTP {}", code));
            json!({"success": false, "error": format!("API Anthropic : {}", detail)})
        }
        Err(e) => json!({"success": false, "error": format!("Connexion a l'API Anthropic impossible : {}", e)}),
    }
}

// ══════════════════════════════════════════════════════════════════
// Utilitaires
// ══════════════════════════════════════════════════════════════════

fn echapper(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn json_response(v: Value) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(v.to_string()).with_header(
        Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap(),
    )
}
