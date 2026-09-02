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
//
// ── Execution d'outils (tool use) ────────────────────────────────
// VexIA peut proposer d'executer une action parmi une liste FERMEE
// (voir tools.rs) : jamais d'acces shell/console libre. Toute action
// qui modifie quelque chose (hors lecture seule) exige une confirmation
// explicite de l'utilisateur avant execution, sauf s'il a active
// "execution automatique" pour les outils a faible risque (jamais les
// outils admin, qui exigent TOUJOURS une confirmation). Chaque
// execution est journalisee dans `vexia_audit`.
// ══════════════════════════════════════════════════════════════════

mod tools;

use crate::appeldb::DbPool;
use crate::c::SessionInfo;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tiny_http::{Header, Request, Response};

const CONFIG_PATH: &str = "config.json";
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";
const DEFAULT_MAX_TOKENS: u64 = 1024;
const MAX_MESSAGE_LEN: usize = 32_000;
const MAX_HISTORY: usize = 20;
const PENDING_TTL_SECS: u64 = 300;
const RATE_WINDOW_SECS: u64 = 60;
const RATE_MAX_PER_WINDOW: usize = 10;

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
            "/status" | "" => statut(pool, session),
            "/chat" => chat(pool, session, req),
            "/confirm" => confirmer(pool, session, req),
            "/prefs" => prefs_update(pool, session, req),
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
                .replace("__USER_ID__", &session.user_id.to_string())
                .replace("__AUTO_CONFIRM__", if prefs.vexia_auto_confirm == 1 { "checked" } else { "" });
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
    provider: String,
    api_key: String,
    cle_perso: bool,
    model: String,
    max_tokens: u64,
    system_prompt: String,
    tools_enabled: bool,
}

/// URL de l'API et modele par defaut pour un fournisseur donne.
/// xAI (Grok) expose une API compatible OpenAI -- meme format de requete/reponse.
fn provider_defaults(provider: &str) -> (&'static str, &'static str) {
    match provider {
        "openai" => ("https://api.openai.com/v1/chat/completions", "gpt-4o-mini"),
        "xai" => ("https://api.x.ai/v1/chat/completions", "grok-2-latest"),
        _ => (ANTHROPIC_API_URL, DEFAULT_MODEL),
    }
}

/// `user_key`/`user_provider` : reglages personnels de l'utilisateur courant
/// (pref.vexia_api_key / vexia_provider), prioritaires sur la config admin
/// partagee si une cle personnelle est renseignee.
fn charger_config(user_key: Option<&str>, user_provider: Option<&str>) -> VexiaConfig {
    let cfg = crate::config_loader::load_config(CONFIG_PATH);
    let params = cfg
        .extensions
        .extension_params
        .get("vexia")
        .and_then(|e| e.get("params"))
        .cloned()
        .unwrap_or(json!({}));

    let admin_key = params
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let admin_provider = params
        .get("provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("anthropic")
        .to_string();
    let user_key = user_key.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (api_key, cle_perso, provider) = match user_key {
        Some(k) => {
            let p = user_provider
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "anthropic".to_string());
            (k, true, p)
        }
        None => (admin_key, false, admin_provider),
    };
    let (_, default_model) = provider_defaults(&provider);
    // Le modele configure par l'admin (params.model) ne s'applique que
    // lorsqu'on utilise sa cle/son fournisseur -- sinon un modele Anthropic
    // configure par l'admin serait envoye a tort a l'API OpenAI/xAI d'un
    // utilisateur ayant choisi un autre fournisseur pour sa cle perso.
    let model = if cle_perso {
        default_model.to_string()
    } else {
        params
            .get("model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(default_model)
            .to_string()
    };

    VexiaConfig {
        provider,
        api_key,
        cle_perso,
        model,
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
        tools_enabled: params
            .get("tools_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    }
}

/// Indique au front si une cle API est configuree, sans jamais l'exposer.
fn statut(pool: &DbPool, session: &SessionInfo) -> Value {
    let prefs = crate::function::get_user_preferences(pool, session.user_id);
    let cfg = charger_config(prefs.vexia_api_key.as_deref(), prefs.vexia_provider.as_deref());
    json!({"success": true, "data": {
        "configure": !cfg.api_key.is_empty(),
        "model": cfg.model,
        "cle_perso": cfg.cle_perso,
        "provider": cfg.provider,
        "tools_actifs": cfg.tools_enabled && cfg.provider == "anthropic",
        "widget_on": prefs.vexia_widget_on != 0,
        "auto_confirm": prefs.vexia_auto_confirm != 0,
    }})
}

// ══════════════════════════════════════════════════════════════════
// Chat — proxy vers l'API Anthropic (Messages API), avec tool use
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

fn tools_json_for(session: &SessionInfo) -> Vec<Value> {
    tools::visible_for(session)
        .iter()
        .map(|t| json!({
            "name": t.name,
            "description": t.description,
            "input_schema": (t.input_schema)(),
        }))
        .collect()
}

fn chat(pool: &DbPool, session: &SessionInfo, req: &mut Request) -> Value {
    let prefs = crate::function::get_user_preferences(pool, session.user_id);
    let cfg = charger_config(prefs.vexia_api_key.as_deref(), prefs.vexia_provider.as_deref());
    if cfg.api_key.is_empty() {
        return json!({"success": false,
            "error": "VexIA n'est pas configure : renseignez votre propre cle API Anthropic dans vos reglages VexIA, ou demandez a un administrateur de configurer la cle partagee."});
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

    // Tool-use n'est cable que pour Anthropic (format de tools specifique) --
    // OpenAI/xAI restent en chat simple pour l'instant.
    let tools_json: Vec<Value> = if cfg.tools_enabled && cfg.provider == "anthropic" { tools_json_for(session) } else { vec![] };
    let messages_sent = messages.clone();

    let body = match llm_call(&cfg, messages, &tools_json) {
        Ok(b) => b,
        Err(e) => return json!({"success": false, "error": e}),
    };

    if body.get("stop_reason").and_then(|v| v.as_str()) == Some("tool_use") {
        let content = body.get("content").cloned().unwrap_or(json!([]));
        if let Some((tool_use_id, name, args)) = premier_tool_use(&content) {
            return traiter_tool_use(pool, session, &cfg, messages_sent, content, tool_use_id, name, args, &tools_json);
        }
    }

    let texte = premier_texte(&body.get("content").cloned().unwrap_or(json!([])));
    if texte.is_empty() {
        json!({"success": false, "error": "Reponse vide du modele."})
    } else {
        json!({"success": true, "reply": texte})
    }
}

/// Decide s'il faut executer tout de suite (lecture seule, ou outil scope
/// avec l'auto-confirm active) ou creer une action en attente qui necessite
/// une confirmation explicite du front (TOUJOURS pour les outils admin).
fn traiter_tool_use(
    pool: &DbPool,
    session: &SessionInfo,
    cfg: &VexiaConfig,
    messages_sent: Vec<Value>,
    assistant_content: Value,
    tool_use_id: String,
    tool_name: String,
    args: Value,
    tools_json: &[Value],
) -> Value {
    let lead_text = premier_texte(&assistant_content);

    let Some(tool) = tools::find(&tool_name) else {
        return json!({"success": false, "error": format!("Outil inconnu : {}", tool_name)});
    };
    // Defense en profondeur : re-verifie le privilege meme si le modele
    // n'aurait normalement pas du recevoir cet outil dans sa liste.
    if !tools::authorized_for(tool, session) {
        return json!({"success": false, "error": "Action non autorisée pour votre niveau de privilège."});
    }

    let prefs = crate::function::get_user_preferences(pool, session.user_id);
    let auto_ok = matches!(tool.tier, tools::ToolTier::Scoped) && prefs.vexia_auto_confirm == 1;

    if tool.read_only || auto_ok {
        if !check_rate_limit(session.user_id) {
            let err = "Trop d'actions exécutées récemment, réessayez dans une minute.".to_string();
            tools::log_tool_execution(pool, session.user_id, tool, &args, &Err(err.clone()));
            return json!({"success": false, "error": err});
        }
        let result = (tool.handler)(pool, session, &args);
        tools::log_tool_execution(pool, session.user_id, tool, &args, &result);
        return suite_apres_execution(cfg, messages_sent, assistant_content, tool_use_id, result, tools_json);
    }

    // Action mutante hors auto-confirm (ou outil admin) : confirmation requise.
    let label = (tool.describe)(&args);
    let id = crate::c::random_hex_id();
    {
        let mut map = pending_store().lock().unwrap();
        prune_pending(&mut map);
        map.insert(id.clone(), PendingAction {
            user_id: session.user_id,
            tool_name: tool_name.clone(),
            args,
            tool_use_id,
            messages_sent,
            assistant_content,
            created_at: Instant::now(),
        });
    }
    json!({
        "success": true,
        "reply": lead_text,
        "pending_action": {
            "id": id,
            "tool": tool_name,
            "tier": tool.tier.as_str(),
            "label": label,
            "expires_in": PENDING_TTL_SECS,
        }
    })
}

/// Envoie le resultat d'un outil a Anthropic (message "tool_result") pour
/// obtenir une reponse en langage naturel a afficher a l'utilisateur.
fn suite_apres_execution(
    cfg: &VexiaConfig,
    messages_sent: Vec<Value>,
    assistant_content: Value,
    tool_use_id: String,
    result: Result<Value, String>,
    tools_json: &[Value],
) -> Value {
    let (result_content, action_ok) = match &result {
        Ok(v) => (v.to_string(), true),
        Err(e) => (json!({"error": e}).to_string(), false),
    };
    let mut followup = messages_sent;
    followup.push(json!({"role": "assistant", "content": assistant_content}));
    followup.push(json!({"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": tool_use_id, "content": result_content, "is_error": !action_ok}
    ]}));

    match llm_call(cfg, followup, tools_json) {
        Ok(body) => {
            let texte = premier_texte(&body.get("content").cloned().unwrap_or(json!([])));
            let reply = if !texte.is_empty() {
                texte
            } else if action_ok {
                "C'est fait.".to_string()
            } else {
                "L'action a échoué.".to_string()
            };
            json!({"success": true, "reply": reply, "action_result": {"success": action_ok}})
        }
        Err(e) => json!({"success": action_ok, "reply": "", "error": e, "action_result": {"success": action_ok}}),
    }
}

// ══════════════════════════════════════════════════════════════════
// Confirmation d'une action en attente
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
struct ConfirmBody {
    action_id: String,
    decision: String,
}

struct PendingAction {
    user_id: i64,
    tool_name: String,
    args: Value,
    tool_use_id: String,
    messages_sent: Vec<Value>,
    assistant_content: Value,
    created_at: Instant,
}

static PENDING: OnceLock<Mutex<HashMap<String, PendingAction>>> = OnceLock::new();

fn pending_store() -> &'static Mutex<HashMap<String, PendingAction>> {
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn prune_pending(map: &mut HashMap<String, PendingAction>) {
    let now = Instant::now();
    map.retain(|_, p| now.duration_since(p.created_at).as_secs() < PENDING_TTL_SECS);
}

fn confirmer(pool: &DbPool, session: &SessionInfo, req: &mut Request) -> Value {
    let mut brut = String::new();
    if std::io::Read::read_to_string(req.as_reader(), &mut brut).is_err() {
        return json!({"success": false, "error": "Corps de requete illisible."});
    }
    let corps: ConfirmBody = match serde_json::from_str(&brut) {
        Ok(c) => c,
        Err(_) => return json!({"success": false, "error": "Requete invalide."}),
    };

    let pending = {
        let mut map = pending_store().lock().unwrap();
        prune_pending(&mut map);
        match map.get(&corps.action_id) {
            Some(p) if p.user_id == session.user_id => map.remove(&corps.action_id),
            _ => None,
        }
    };

    let Some(pending) = pending else {
        return json!({"success": false, "error": "Action expirée ou introuvable."});
    };

    if corps.decision != "confirm" {
        return json!({"success": true, "cancelled": true});
    }

    let Some(tool) = tools::find(&pending.tool_name) else {
        return json!({"success": false, "error": "Outil inconnu."});
    };
    // Re-verification live : le privilege a pu changer entre la proposition
    // et la confirmation.
    if !tools::authorized_for(tool, session) {
        return json!({"success": false, "error": "Action non autorisée pour votre niveau de privilège."});
    }
    if !check_rate_limit(session.user_id) {
        let err = "Trop d'actions exécutées récemment, réessayez dans une minute.".to_string();
        tools::log_tool_execution(pool, session.user_id, tool, &pending.args, &Err(err.clone()));
        return json!({"success": false, "error": err});
    }

    // Execute avec UNIQUEMENT les arguments stockes cote serveur -- jamais
    // ceux (absents ici) du corps de la requete de confirmation.
    let result = (tool.handler)(pool, session, &pending.args);
    tools::log_tool_execution(pool, session.user_id, tool, &pending.args, &result);

    let prefs = crate::function::get_user_preferences(pool, session.user_id);
    let cfg = charger_config(prefs.vexia_api_key.as_deref(), prefs.vexia_provider.as_deref());
    let tools_json = tools_json_for(session);
    suite_apres_execution(&cfg, pending.messages_sent, pending.assistant_content, pending.tool_use_id, result, &tools_json)
}

// ══════════════════════════════════════════════════════════════════
// Preference "execution automatique" (outils scoped uniquement)
// ══════════════════════════════════════════════════════════════════

#[derive(Deserialize, Default)]
struct PrefsBody {
    auto_confirm: Option<bool>,
    api_key: Option<String>,
    provider: Option<String>,
    widget_on: Option<bool>,
}

fn prefs_update(pool: &DbPool, session: &SessionInfo, req: &mut Request) -> Value {
    let mut brut = String::new();
    if std::io::Read::read_to_string(req.as_reader(), &mut brut).is_err() {
        return json!({"success": false, "error": "Corps de requete illisible."});
    }
    let corps: PrefsBody = match serde_json::from_str(&brut) {
        Ok(c) => c,
        Err(_) => return json!({"success": false, "error": "Requete invalide."}),
    };
    let mut ok = true;
    if let Some(auto) = corps.auto_confirm {
        ok &= crate::function::update_user_preference(
            pool, session.user_id, "vexia_auto_confirm", if auto { "1" } else { "0" },
        );
    }
    if let Some(cle) = corps.api_key {
        // Chaine vide = retire la cle personnelle, retombe sur la cle admin.
        ok &= crate::function::update_user_preference(pool, session.user_id, "vexia_api_key", cle.trim());
    }
    if let Some(provider) = corps.provider {
        let p = match provider.trim() {
            "openai" | "xai" => provider.trim(),
            _ => "anthropic",
        };
        ok &= crate::function::update_user_preference(pool, session.user_id, "vexia_provider", p);
    }
    if let Some(on) = corps.widget_on {
        ok &= crate::function::update_user_preference(
            pool, session.user_id, "vexia_widget_on", if on { "1" } else { "0" },
        );
    }
    if ok {
        json!({"success": true})
    } else {
        json!({"success": false, "error": "Echec de la sauvegarde."})
    }
}

// ══════════════════════════════════════════════════════════════════
// Limite de frequence d'execution d'outils (independante du chat lui-meme)
// ══════════════════════════════════════════════════════════════════

static EXEC_RATE: OnceLock<Mutex<HashMap<i64, VecDeque<Instant>>>> = OnceLock::new();

fn check_rate_limit(user_id: i64) -> bool {
    let store = EXEC_RATE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = store.lock().unwrap();
    let now = Instant::now();
    let dq = map.entry(user_id).or_default();
    while dq.front().map_or(false, |t: &Instant| now.duration_since(*t).as_secs() > RATE_WINDOW_SECS) {
        dq.pop_front();
    }
    if dq.len() >= RATE_MAX_PER_WINDOW {
        return false;
    }
    dq.push_back(now);
    true
}

// ══════════════════════════════════════════════════════════════════
// Appel Anthropic bas niveau + extraction de contenu
// ══════════════════════════════════════════════════════════════════

/// Appelle le fournisseur configure. Le tool-use n'est cable que pour
/// Anthropic (`tools` est ignore pour les autres) -- OpenAI/xAI repondent
/// donc toujours en `{"content":[{"type":"text","text":...}]}`, normalise
/// pour que le reste du code (premier_texte/premier_tool_use, verif du
/// stop_reason) n'ait pas a connaitre le fournisseur actif.
fn llm_call(cfg: &VexiaConfig, messages: Vec<Value>, tools: &[Value]) -> Result<Value, String> {
    match cfg.provider.as_str() {
        "openai" | "xai" => appel_compatible_openai(cfg, messages),
        _ => appel_anthropic(cfg, messages, tools),
    }
}

fn nom_fournisseur(provider: &str) -> &'static str {
    match provider {
        "openai" => "OpenAI",
        "xai" => "xAI (Grok)",
        _ => "Anthropic",
    }
}

fn appel_anthropic(cfg: &VexiaConfig, messages: Vec<Value>, tools: &[Value]) -> Result<Value, String> {
    let mut payload = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "system": cfg.system_prompt,
        "messages": messages,
    });
    if !tools.is_empty() {
        payload["tools"] = json!(tools);
    }

    let resp = ureq::post(ANTHROPIC_API_URL)
        .set("x-api-key", &cfg.api_key)
        .set("anthropic-version", ANTHROPIC_VERSION)
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(60))
        .send_json(payload);

    match resp {
        Ok(r) => Ok(r.into_json().unwrap_or(json!({}))),
        Err(ureq::Error::Status(code, r)) => {
            let detail = extraire_erreur(r, code);
            Err(format!("API Anthropic : {}", detail))
        }
        Err(e) => Err(format!("Connexion a l'API Anthropic impossible : {}", e)),
    }
}

/// OpenAI et xAI (Grok) exposent la meme API Chat Completions.
fn appel_compatible_openai(cfg: &VexiaConfig, messages: Vec<Value>) -> Result<Value, String> {
    let (url, _) = provider_defaults(&cfg.provider);
    let mut msgs = vec![json!({"role": "system", "content": cfg.system_prompt})];
    msgs.extend(messages);
    let payload = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "messages": msgs,
    });

    let resp = ureq::post(url)
        .set("Authorization", &format!("Bearer {}", cfg.api_key))
        .set("content-type", "application/json")
        .timeout(Duration::from_secs(60))
        .send_json(payload);

    let nom = nom_fournisseur(&cfg.provider);
    match resp {
        Ok(r) => {
            let body: Value = r.into_json().unwrap_or(json!({}));
            let texte = body
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            Ok(json!({"content": [{"type": "text", "text": texte}]}))
        }
        Err(ureq::Error::Status(code, r)) => {
            let detail = extraire_erreur(r, code);
            Err(format!("API {} : {}", nom, detail))
        }
        Err(e) => Err(format!("Connexion a l'API {} impossible : {}", nom, e)),
    }
}

fn extraire_erreur(r: ureq::Response, code: u16) -> String {
    r.into_json::<Value>()
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.get("message").or(Some(e))).and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| format!("HTTP {}", code))
}

fn premier_texte(content: &Value) -> String {
    content
        .as_array()
        .and_then(|arr| arr.iter().find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text")))
        .and_then(|b| b.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string()
}

fn premier_tool_use(content: &Value) -> Option<(String, String, Value)> {
    content.as_array().and_then(|arr| {
        arr.iter()
            .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .map(|b| (
                b.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                b.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                b.get("input").cloned().unwrap_or(json!({})),
            ))
    })
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
