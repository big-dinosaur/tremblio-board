use axum::Router;
use axum::routing::{get, post};

use crate::AppState;

mod auth;
mod boards;
mod posts;
mod threads;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        .route("/api/boards", get(boards::list).post(boards::create))
        .route(
            "/api/boards/{id}",
            get(boards::get).put(boards::update).delete(boards::delete),
        )
        .route(
            "/api/boards/{id}/threads",
            get(threads::list).post(threads::create),
        )
        .route(
            "/api/threads/{id}",
            get(threads::get)
                .put(threads::update)
                .delete(threads::delete),
        )
        .route(
            "/api/threads/{id}/posts",
            get(posts::list).post(posts::create),
        )
        .route(
            "/api/posts/{id}",
            get(posts::get).put(posts::update).delete(posts::delete),
        )
}
