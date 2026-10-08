use axum::http::StatusCode;
use serde_json::json;

use super::{PASSWORD, TestApp};

#[tokio::test]
async fn first_registered_user_becomes_admin() {
    let app = TestApp::new().await;
    let alice = app.register("alice").await;
    assert_eq!(alice["is_admin"], true);
    assert_eq!(alice["is_banned"], false);

    let bob = app.register("bob").await;
    assert_eq!(bob["is_admin"], false);
}

#[tokio::test]
async fn register_trims_username_and_hides_password_hash() {
    let app = TestApp::new().await;
    let (status, body) = app
        .post(
            "/api/auth/register",
            None,
            json!({ "username": "  carol  ", "password": PASSWORD, "nickname": "卡罗尔" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["username"], "carol");
    assert_eq!(body["nickname"], "卡罗尔");
    assert!(body.get("password_hash").is_none());

    // 未提供昵称时使用用户名
    let dave = app.register("dave").await;
    assert_eq!(dave["nickname"], "dave");
}

#[tokio::test]
async fn register_rejects_invalid_input() {
    let app = TestApp::new().await;
    let long_name = "a".repeat(33);
    let cases = [
        ("ab", PASSWORD),
        (long_name.as_str(), PASSWORD),
        ("bad name", PASSWORD),
        ("名字abc", PASSWORD),
        ("erin", "12345"),
    ];
    for (username, password) in cases {
        let (status, body) = app
            .post(
                "/api/auth/register",
                None,
                json!({ "username": username, "password": password }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{username:?} 应被拒绝");
        assert!(body["error"].is_string());
    }
}

#[tokio::test]
async fn register_rejects_duplicate_username() {
    let app = TestApp::new().await;
    app.register("alice").await;
    let (status, _) = app
        .post(
            "/api/auth/register",
            None,
            json!({ "username": "alice", "password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn login_returns_token_usable_for_me() {
    let app = TestApp::new().await;
    app.register("alice").await;
    let (status, body) = app
        .post(
            "/api/auth/login",
            None,
            json!({ "username": "alice", "password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["user"]["last_login_at"].as_i64().unwrap() > 0);
    let token = body["token"].as_str().unwrap();

    let (status, me) = app.get("/api/auth/me", Some(token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "alice");
    assert!(me.get("password_hash").is_none());
}

#[tokio::test]
async fn login_rejects_wrong_password_and_unknown_user() {
    let app = TestApp::new().await;
    app.register("alice").await;
    for (username, password) in [("alice", "wrong-password"), ("nobody", PASSWORD)] {
        let (status, _) = app
            .post(
                "/api/auth/login",
                None,
                json!({ "username": username, "password": password }),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn me_requires_valid_token() {
    let app = TestApp::new().await;
    let (status, _) = app.get("/api/auth/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.get("/api/auth/me", Some("not-a-real-token")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_revokes_token() {
    let app = TestApp::new().await;
    let token = app.user("alice").await;
    let (status, _) = app
        .call(axum::http::Method::POST, "/api/auth/logout", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = app.get("/api/auth/me", Some(&token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_token_is_rejected() {
    let app = TestApp::new().await;
    let token = app.user("alice").await;
    sqlx::query("UPDATE bbs_tokens SET expires_at = 0")
        .execute(&app.db)
        .await
        .unwrap();
    let (status, _) = app.get("/api/auth/me", Some(&token)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn banned_user_cannot_login_or_use_existing_token() {
    let app = TestApp::new().await;
    let token = app.user("alice").await;
    sqlx::query("UPDATE bbs_users SET is_banned = 1 WHERE username = 'alice'")
        .execute(&app.db)
        .await
        .unwrap();

    let (status, _) = app.get("/api/auth/me", Some(&token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app
        .post(
            "/api/auth/login",
            None,
            json!({ "username": "alice", "password": PASSWORD }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
