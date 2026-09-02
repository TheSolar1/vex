// ══════════════════════════════════════════════════════════════════
// admin/actions.rs — logique d'action admin, partagee entre le panel
// HTTP humain (admin.rs) et le dispatcher d'outils VexIA.
//
// Les fonctions ici sont la SEULE source de verite pour les regles
// d'autorisation (pas d'auto-cible, pas d'elevation au-dela du
// privilege de l'appelant, etc.) : ni admin.rs ni vexia ne doivent
// reimplementer ces checks en parallele.
// ══════════════════════════════════════════════════════════════════

use crate::appeldb::{inserer_ou_modifier, selectionner, supprimer_ligne, DbPool};
use serde_json::{json, Value};

pub(crate) const PRIVILEGE_MAX: i64 = 3;
pub(crate) const PRIVILEGE_SUPER: i64 = 2;
pub(crate) const PRIVILEGE_MIN_SET: i64 = 2;

/// Change le privilege d'un utilisateur. Regles : pas d'attribution du
/// privilege 1 (fondateur) via ce chemin, pas d'auto-cible, et un appelant
/// non-superadmin ne peut jamais attribuer un privilege plus fort que le sien.
pub(crate) fn set_user_privilege(
    pool: &DbPool,
    caller_id: i64,
    caller_privilege: i64,
    target_id: i64,
    new_privilege: i64,
) -> Result<Value, String> {
    if new_privilege < PRIVILEGE_MIN_SET {
        return Err("Le privilege 1 ne peut pas être attribué via le panel.".into());
    }
    if new_privilege > 12 || target_id == caller_id {
        return Err("Action non autorisée.".into());
    }
    if caller_privilege > PRIVILEGE_SUPER && new_privilege < caller_privilege {
        return Err("Vous ne pouvez pas donner un privilege supérieur au vôtre.".into());
    }
    inserer_ou_modifier(
        pool,
        "login",
        &[("privilege", mysql::Value::from(new_privilege))],
        &[("id", mysql::Value::from(target_id))],
    );
    Ok(json!({
        "success": true,
        "message": "Privilège mis à jour.",
        "uid": target_id,
        "privilege": new_privilege,
    }))
}

/// Supprime un compte utilisateur. Regles : pas d'auto-suppression, et un
/// appelant non-superadmin ne peut pas supprimer un superadmin.
pub(crate) fn delete_user(
    pool: &DbPool,
    caller_id: i64,
    caller_privilege: i64,
    target_id: i64,
) -> Result<Value, String> {
    if target_id == caller_id {
        return Err("Impossible de supprimer votre propre compte.".into());
    }
    let target = selectionner(
        pool,
        "login",
        &[("id", mysql::Value::from(target_id))],
        &["privilege"],
        None,
        Some(1),
    );
    let target_priv = target
        .first()
        .and_then(|r| r.get("privilege"))
        .and_then(|v| v.as_i64())
        .unwrap_or(99);
    if target_priv <= PRIVILEGE_SUPER && caller_privilege > PRIVILEGE_SUPER {
        return Err("Impossible de supprimer un superadmin.".into());
    }
    supprimer_ligne(pool, "login", "id", mysql::Value::from(target_id));
    Ok(json!({
        "success": true,
        "message": "Utilisateur supprimé.",
        "uid": target_id,
    }))
}
