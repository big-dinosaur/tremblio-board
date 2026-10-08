use axum::http::StatusCode;
use serde_json::json;

use super::TestApp;

/// 管理员、话题作者、路人三个账号，以及一个板块和一个话题
struct Fixture {
    app: TestApp,
    admin: String,
    author: String,
    other: String,
    board_id: i64,
    thread_id: i64,
    first_post_id: i64,
}

async fn fixture() -> Fixture {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let author = app.user("author").await;
    let other = app.user("other").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (thread_id, first_post_id) = app.create_thread(&author, board_id, "话题").await;
    Fixture {
        app,
        admin,
        author,
        other,
        board_id,
        thread_id,
        first_post_id,
    }
}

#[tokio::test]
async fn reply_updates_thread_and_board_counts() {
    let f = fixture().await;
    let (status, post) = f
        .app
        .post(
            &format!("/api/threads/{}/posts", f.thread_id),
            Some(&f.other),
            json!({ "content": "沙发" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(post["thread_id"], f.thread_id);
    assert_eq!(post["author"], "other");
    assert_eq!(post["content"], "沙发");

    let thread = f.app.thread(f.thread_id).await;
    assert_eq!(thread["reply_count"], 1);
    assert_eq!(thread["last_post_at"], post["created_at"]);
    assert_eq!(f.app.board(f.board_id).await["post_count"], 2);
}

#[tokio::test]
async fn reply_rejects_bad_requests() {
    let f = fixture().await;
    let uri = format!("/api/threads/{}/posts", f.thread_id);

    let (status, _) = f.app.post(&uri, None, json!({ "content": "x" })).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = f
        .app
        .post(&uri, Some(&f.other), json!({ "content": "   " }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = f
        .app
        .post(
            "/api/threads/9999/posts",
            Some(&f.other),
            json!({ "content": "x" }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    assert_eq!(f.app.thread(f.thread_id).await["reply_count"], 0);
}

#[tokio::test]
async fn list_posts_in_order_and_paginates() {
    let f = fixture().await;
    let r1 = f.app.reply(&f.other, f.thread_id, "1楼").await;
    let r2 = f.app.reply(&f.author, f.thread_id, "2楼").await;
    let r3 = f.app.reply(&f.other, f.thread_id, "3楼").await;

    let ids = |body: &serde_json::Value| -> Vec<i64> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_i64().unwrap())
            .collect()
    };
    let uri = format!("/api/threads/{}/posts", f.thread_id);
    let (status, page1) = f.app.get(&format!("{uri}?per_page=2"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page1["total"], 4);
    assert_eq!(ids(&page1), [f.first_post_id, r1]);

    let (_, page2) = f.app.get(&format!("{uri}?per_page=2&page=2"), None).await;
    assert_eq!(ids(&page2), [r2, r3]);

    let (status, _) = f.app.get("/api/threads/9999/posts", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn locked_thread_only_accepts_admin_replies() {
    let f = fixture().await;
    let (status, _) = f
        .app
        .put(
            &format!("/api/threads/{}", f.thread_id),
            Some(&f.admin),
            json!({ "is_locked": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let uri = format!("/api/threads/{}/posts", f.thread_id);
    for token in [&f.author, &f.other] {
        let (status, _) = f
            .app
            .post(&uri, Some(token), json!({ "content": "还能回吗" }))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    f.app.reply(&f.admin, f.thread_id, "管理员可以").await;
}

#[tokio::test]
async fn only_owner_or_admin_can_edit_post() {
    let f = fixture().await;
    let reply_id = f.app.reply(&f.other, f.thread_id, "原内容").await;
    let uri = format!("/api/posts/{reply_id}");

    let (status, body) = f
        .app
        .put(&uri, Some(&f.other), json!({ "content": "改过的内容" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["content"], "改过的内容");

    let (status, _) = f
        .app
        .put(&uri, Some(&f.author), json!({ "content": "话题作者改的" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = f
        .app
        .put(&uri, Some(&f.admin), json!({ "content": "管理员改的" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["content"], "管理员改的");

    let (status, _) = f
        .app
        .put(&uri, Some(&f.other), json!({ "content": "" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(f.app.get(&uri, None).await.1["content"], "管理员改的");
}

#[tokio::test]
async fn first_post_cannot_be_deleted_alone() {
    let f = fixture().await;
    let (status, _) = f
        .app
        .delete(&format!("/api/posts/{}", f.first_post_id), Some(&f.author))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = f
        .app
        .get(&format!("/api/posts/{}", f.first_post_id), None)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn delete_reply_updates_thread_and_board_counts() {
    let f = fixture().await;
    let keep_id = f.app.reply(&f.author, f.thread_id, "保留").await;
    let reply_id = f.app.reply(&f.other, f.thread_id, "删除").await;
    let uri = format!("/api/posts/{reply_id}");

    let (status, _) = f.app.delete(&uri, Some(&f.author)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = f.app.delete(&uri, Some(&f.other)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(f.app.get(&uri, None).await.0, StatusCode::NOT_FOUND);

    let kept = f.app.get(&format!("/api/posts/{keep_id}"), None).await.1;
    let thread = f.app.thread(f.thread_id).await;
    assert_eq!(thread["reply_count"], 1);
    assert_eq!(thread["last_post_at"], kept["created_at"]);
    assert_eq!(f.app.board(f.board_id).await["post_count"], 2);

    let (status, _) = f.app.delete(&uri, Some(&f.other)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
