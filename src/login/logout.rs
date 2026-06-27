// ══════════════════════════════════════════════════════════════════
// login/logout.rs — VEX Logout
//
// Méthode : GET /login/logout
//   1. Lit le cookie "connexion_cookie"
//   2. Supprime la ligne correspondante dans la table "loginc"
//   3. Expire le cookie côté client (Max-Age=0)
//   4. Redirige vers /login/login
//
// Sécurité :
//   - Vérifie la session via c::verifier_session avant suppression
//     (évite de supprimer une ligne avec un cookie forgé arbitraire)
//   - Réponse identique qu'il y ait un cookie valide ou non
//     (pas de fuite d'information)
//   - Même flux pour GET (page HTML) et POST (appel API/JS)
// ══════════════════════════════════════════════════════════════════

use crate::access_control::{get_cookie, get_header};
use crate::appeldb::{supprimer_ligne, DbPool};
use crate::utils::strip_port;
use tiny_http::{Request, Response};

pub fn handle_request(request: Request, pool: &DbPool, remote_full: &str) {
    let remote_ip = strip_port(remote_full);
    let method = request.method().to_string();
    let cookie_val = get_cookie(&request, "connexion_cookie");
    let user_agent = get_header(&request, "User-Agent");

    // ── Pas de cookie → sert juste la page (ou redirige) ─────────
    if cookie_val.is_empty() {
        if method == "POST" {
            respond_json_redirect(request, "/login/login");
        } else {
            // GET sans cookie : redirige directement
            redirect(request, "/login/login");
        }
        return;
    }

    // ── Vérifie que le cookie correspond bien à une session active ─
    // Cela évite qu'un attaquant supprime une ligne arbitraire de loginc
    // en forgeant une valeur de cookie.
    let session = crate::c::verifier_session(pool, &cookie_val, &remote_ip, &user_agent);

    if session.connecte {
        // Supprime la ligne de session en DB
        supprimer_ligne(
            pool,
            "loginc",
            "idcokier",
            mysql::Value::from(cookie_val.as_str()),
        );
    }
    // Si session invalide, on expire quand même le cookie côté client
    // (nettoyage) — pas d'erreur exposée.

    // ── Expire le cookie côté client ─────────────────────────────
    // Max-Age=0 supprime immédiatement le cookie dans le navigateur.
    // On conserve les mêmes attributs que lors du Set-Cookie au login
    // (Path, HttpOnly, SameSite) pour que le navigateur le retrouve.
    // Attributs identiques à ceux du Set-Cookie dans login.rs (Path=/, HttpOnly).
    // SameSite n'est PAS ajouté ici car login.rs ne le met pas non plus :
    // le navigateur retrouve le cookie à expirer uniquement si les attributs Path
    // et Secure correspondent — Max-Age=0 suffit à le supprimer côté client.
    let expire_cookie = "connexion_cookie=; Path=/; HttpOnly; Max-Age=0";

    if method == "POST" {
        // Appel API (ex: bouton logout JS) → JSON + cookie expiré
        let body = r#"{"success":true,"redirect":"/login/login"}"#;
        let _ = request.respond(
            Response::from_string(body)
                .with_status_code(200)
                .with_header(
                    tiny_http::Header::from_bytes(
                        "Content-Type",
                        "application/json; charset=utf-8",
                    )
                    .unwrap(),
                )
                .with_header(tiny_http::Header::from_bytes("Set-Cookie", expire_cookie).unwrap()),
        );
    } else {
        // GET → sert la page HTML statique (affiche "déconnexion…" puis redirige)
        match std::fs::read_to_string("static/login/logout.html") {
            Ok(html) => {
                let _ = request.respond(
                    Response::from_string(html)
                        .with_status_code(200)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                "Content-Type",
                                "text/html; charset=utf-8",
                            )
                            .unwrap(),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes("Set-Cookie", expire_cookie).unwrap(),
                        ),
                );
            }
            Err(_) => {
                // Fallback si le HTML est introuvable : redirige directement
                let _ = request.respond(
                    Response::empty(302)
                        .with_header(
                            tiny_http::Header::from_bytes("Location", "/login/login").unwrap(),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes("Set-Cookie", expire_cookie).unwrap(),
                        ),
                );
            }
        }
    }
}

// ── Utilitaires locaux ────────────────────────────────────────────

fn redirect(request: Request, location: &str) {
    let _ = request.respond(
        Response::empty(302)
            .with_header(tiny_http::Header::from_bytes("Location", location).unwrap()),
    );
}

fn respond_json_redirect(request: Request, location: &str) {
    let body = format!(r#"{{"success":false,"redirect":"{}"}}"#, location);
    let _ = request.respond(
        Response::from_string(body)
            .with_status_code(200)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            ),
    );
}
