// ══════════════════════════════════════════════════════════════════
// extensions/vexia/tools.rs — registre d'outils que VexIA peut appeler
// via l'API "tool use" d'Anthropic.
//
// Regles de securite (ne jamais contourner) :
//   - Liste FERMEE et explicite : pas d'execution shell/console libre.
//   - Chaque handler recoit la session de l'appelant et applique EXACTEMENT
//     les memes controles de privilege que l'endpoint HTTP humain
//     equivalent, en appelant la meme fonction partagee (voir
//     crate::admin::actions et crate::mess::mess).
//   - `describe()` produit le libelle de confirmation cote serveur, jamais
//     a partir du texte du modele (defense contre le prompt injection).
// ══════════════════════════════════════════════════════════════════

use crate::admin::actions::{self, PRIVILEGE_SUPER};
use crate::appeldb::{inserer_ou_modifier, DbPool};
use crate::c::SessionInfo;
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolTier {
    Scoped,
    Admin,
}

impl ToolTier {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ToolTier::Scoped => "scoped",
            ToolTier::Admin => "admin",
        }
    }
}

pub(crate) struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub tier: ToolTier,
    pub read_only: bool,
    pub input_schema: fn() -> Value,
    pub handler: fn(&DbPool, &SessionInfo, &Value) -> Result<Value, String>,
    pub describe: fn(&Value) -> String,
}

pub(crate) fn registry() -> &'static [ToolSpec] {
    &[
        ToolSpec {
            name: "admin_set_user_privilege",
            description: "Change le niveau de privilege d'un utilisateur (reserve fondateur/superadmin).",
            tier: ToolTier::Admin,
            read_only: false,
            input_schema: || json!({
                "type": "object",
                "properties": {
                    "target_user_id": {"type": "integer", "description": "id de l'utilisateur cible"},
                    "new_privilege": {"type": "integer", "description": "nouveau niveau de privilege (1=fondateur .. 12)"},
                },
                "required": ["target_user_id", "new_privilege"],
            }),
            handler: tool_admin_set_user_privilege,
            describe: |args| format!(
                "Changer le privilege de l'utilisateur #{} vers {}",
                args.get("target_user_id").and_then(|v| v.as_i64()).unwrap_or(0),
                args.get("new_privilege").and_then(|v| v.as_i64()).unwrap_or(0),
            ),
        },
        ToolSpec {
            name: "admin_delete_user",
            description: "Supprime definitivement un compte utilisateur (reserve fondateur/superadmin).",
            tier: ToolTier::Admin,
            read_only: false,
            input_schema: || json!({
                "type": "object",
                "properties": {
                    "target_user_id": {"type": "integer", "description": "id de l'utilisateur a supprimer"},
                },
                "required": ["target_user_id"],
            }),
            handler: tool_admin_delete_user,
            describe: |args| format!(
                "Supprimer l'utilisateur #{}",
                args.get("target_user_id").and_then(|v| v.as_i64()).unwrap_or(0),
            ),
        },
        ToolSpec {
            name: "mess_list_my_messages",
            description: "Liste les messages de l'utilisateur courant dans un dossier (inbox, sent ou trash). Lecture seule.",
            tier: ToolTier::Scoped,
            read_only: true,
            input_schema: || json!({
                "type": "object",
                "properties": {
                    "folder": {"type": "string", "enum": ["inbox", "sent", "trash"], "description": "dossier a lister, defaut inbox"},
                },
            }),
            handler: tool_mess_list_my_messages,
            describe: |args| format!(
                "Lister mes messages ({})",
                args.get("folder").and_then(|v| v.as_str()).unwrap_or("inbox"),
            ),
        },
        ToolSpec {
            name: "mess_delete_my_message",
            description: "Supprime (corbeille) un message de l'utilisateur courant.",
            tier: ToolTier::Scoped,
            read_only: false,
            input_schema: || json!({
                "type": "object",
                "properties": {
                    "message_id": {"type": "integer", "description": "id du message a supprimer"},
                },
                "required": ["message_id"],
            }),
            handler: tool_mess_delete_my_message,
            describe: |args| format!(
                "Supprimer mon message #{}",
                args.get("message_id").and_then(|v| v.as_i64()).unwrap_or(0),
            ),
        },
    ]
}

pub(crate) fn find(name: &str) -> Option<&'static ToolSpec> {
    registry().iter().find(|t| t.name == name)
}

/// Outils visibles par l'appelant, filtres par privilege — les outils
/// "admin" ne sont jamais proposes au modele pour un appelant non-superadmin.
pub(crate) fn visible_for(session: &SessionInfo) -> Vec<&'static ToolSpec> {
    registry().iter().filter(|t| authorized_for(t, session)).collect()
}

/// Verification de privilege, appelee a la fois pour filtrer les outils
/// proposes au modele ET, en defense en profondeur, avant toute execution
/// reelle (proposition ou confirmation) -- jamais fait confiance a un seul
/// endroit.
pub(crate) fn authorized_for(tool: &ToolSpec, session: &SessionInfo) -> bool {
    tool.tier != ToolTier::Admin || session.user_privilege <= PRIVILEGE_SUPER
}

fn tool_admin_set_user_privilege(pool: &DbPool, session: &SessionInfo, args: &Value) -> Result<Value, String> {
    let target_id = args.get("target_user_id").and_then(|v| v.as_i64()).ok_or("target_user_id manquant")?;
    let new_privilege = args.get("new_privilege").and_then(|v| v.as_i64()).ok_or("new_privilege manquant")?;
    actions::set_user_privilege(pool, session.user_id, session.user_privilege, target_id, new_privilege)
}

fn tool_admin_delete_user(pool: &DbPool, session: &SessionInfo, args: &Value) -> Result<Value, String> {
    let target_id = args.get("target_user_id").and_then(|v| v.as_i64()).ok_or("target_user_id manquant")?;
    actions::delete_user(pool, session.user_id, session.user_privilege, target_id)
}

fn tool_mess_list_my_messages(pool: &DbPool, session: &SessionInfo, args: &Value) -> Result<Value, String> {
    let folder = args.get("folder").and_then(|v| v.as_str()).unwrap_or("inbox");
    let mut conn = pool.get_conn().map_err(|e| format!("Connexion base impossible : {e}"))?;
    let mut v = crate::mess::mess::query_messages(&mut conn, session, folder);
    // Le contenu chiffre n'a aucun sens pour le modele -- on ne l'expose pas.
    if let Some(arr) = v.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for m in arr.iter_mut() {
            if let Some(o) = m.as_object_mut() {
                o.remove("subj_enc");
                o.remove("body_enc");
            }
        }
    }
    Ok(v)
}

fn tool_mess_delete_my_message(pool: &DbPool, session: &SessionInfo, args: &Value) -> Result<Value, String> {
    let id = args.get("message_id").and_then(|v| v.as_i64()).ok_or("message_id manquant")?;
    let mut conn = pool.get_conn().map_err(|e| format!("Connexion base impossible : {e}"))?;
    crate::mess::mess::delete_message(&mut conn, session, id)
}

/// Journal d'audit : une ligne par execution d'outil (reussie ou non).
pub(crate) fn log_tool_execution(
    pool: &DbPool,
    user_id: i64,
    tool: &ToolSpec,
    args: &Value,
    result: &Result<Value, String>,
) {
    let (success, result_json, error): (i64, String, String) = match result {
        Ok(v) => (1, v.to_string(), String::new()),
        Err(e) => (0, String::new(), e.clone()),
    };
    inserer_ou_modifier(
        pool,
        "vexia_audit",
        &[
            ("user_id", mysql::Value::from(user_id)),
            ("tool_name", mysql::Value::from(tool.name)),
            ("tier", mysql::Value::from(tool.tier.as_str())),
            ("args_json", mysql::Value::from(args.to_string())),
            ("success", mysql::Value::from(success)),
            ("result_json", mysql::Value::from(result_json)),
            ("error", mysql::Value::from(error)),
        ],
        &[],
    );
}
