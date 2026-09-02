// ══════════════════════════════════════════════════════════════════
// extensions/mod.rs — FICHIER GENERE AUTOMATIQUEMENT
// Reecrit par le panel admin VEX (section Extensions) a chaque
// ajout ou suppression d'extension. Ne pas editer a la main.
//
// Convention : chaque extension vit dans src/extensions/<id>/mod.rs
// et expose :
//     pub fn handle(pool: &DbPool, session: &SessionInfo,
//                   req: &mut Request) -> Response<Cursor<Vec<u8>>>
// Elle est ensuite servie sur /ext/<id> et /api/ext/<id>, apres
// verification du privilege et du plan definis dans config.json.
// ══════════════════════════════════════════════════════════════════

#![allow(unused_imports, unused_variables, dead_code)]

use crate::appeldb::DbPool;
use crate::c::SessionInfo;
use std::io::Cursor;
use tiny_http::{Request, Response};

pub mod qseal;
pub mod vexia;

/// Extensions reellement compilees dans ce binaire.
pub fn compiled_ids() -> &'static [&'static str] {
    &["qseal", "vexia"]
}

/// Route /ext/<id> vers l'extension. None si l'id n'est pas compile.
pub fn dispatch(
    id: &str,
    pool: &DbPool,
    session: &SessionInfo,
    req: &mut Request,
) -> Option<Response<Cursor<Vec<u8>>>> {
    match id {
        "qseal" => Some(qseal::handle(pool, session, req)),
        "vexia" => Some(vexia::handle(pool, session, req)),
        _ => None,
    }
}
