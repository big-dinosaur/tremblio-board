use axum::http::StatusCode;
use serde_json::json;

use super::TestApp;

#[tokio::test]
async fn create_thread_creates_first_post_and_updates_board_counts() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let member = app.user("member").await;
    let board_id = app.create_board(&admin, "综合").await;

    let (status, body) = app
        .post(
            &format!("/api/boards/{board_id}/threads"),
            Some(&member),
            json!({ "title": "  第一个话题  ", "content": "大家好" }),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let thread = &body["thread"];
    assert_eq!(thread["title"], "第一个话题");
    assert_eq!(thread["author"], "member");
    assert_eq!(thread["board_id"], board_id);
    assert_eq!(thread["reply_count"], 0);
    assert_eq!(thread["is_pinned"], false);
    assert_eq!(thread["is_locked"], false);
    let first_post = &body["first_post"];
    assert_eq!(first_post["thread_id"], thread["id"]);
    assert_eq!(first_post["content"], "大家好");

    let board = app.board(board_id).await;
    assert_eq!(board["thread_count"], 1);
    assert_eq!(board["post_count"], 1);
}

#[tokio::test]
async fn create_thread_rejects_bad_requests() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let board_id = app.create_board(&admin, "综合").await;
    let uri = format!("/api/boards/{board_id}/threads");
    let ok = json!({ "title": "标题", "content": "内容" });

    let (status, _) = app.post(&uri, None, ok.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app
        .post(&uri, Some(&admin), json!({ "title": " ", "content": "内容" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .post(&uri, Some(&admin), json!({ "title": "标题", "content": "" }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = app
        .post("/api/boards/9999/threads", Some(&admin), ok)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    assert_eq!(app.board(board_id).await["thread_count"], 0);
}

#[tokio::test]
async fn list_threads_puts_pinned_first_and_paginates() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (t1, _) = app.create_thread(&admin, board_id, "t1").await;
    let (t2, _) = app.create_thread(&admin, board_id, "t2").await;
    let (t3, _) = app.create_thread(&admin, board_id, "t3").await;
    let (status, _) = app
        .put(
            &format!("/api/threads/{t1}"),
            Some(&admin),
            json!({ "is_pinned": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let ids = |body: &serde_json::Value| -> Vec<i64> {
        body["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_i64().unwrap())
            .collect()
    };
    let uri = format!("/api/boards/{board_id}/threads");
    let (status, page1) = app.get(&format!("{uri}?per_page=2"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page1["total"], 3);
    assert_eq!(page1["page"], 1);
    assert_eq!(page1["per_page"], 2);
    assert_eq!(ids(&page1), [t1, t3]);

    let (_, page2) = app.get(&format!("{uri}?per_page=2&page=2"), None).await;
    assert_eq!(ids(&page2), [t2]);

    let (status, _) = app.get("/api/boards/9999/threads", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_thread_increments_view_count() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (thread_id, _) = app.create_thread(&admin, board_id, "话题").await;

    assert_eq!(app.thread(thread_id).await["view_count"], 1);
    assert_eq!(app.thread(thread_id).await["view_count"], 2);

    let (status, _) = app.get("/api/threads/9999", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_owner_or_admin_can_edit_title() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let author = app.user("author").await;
    let other = app.user("other").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (thread_id, _) = app.create_thread(&author, board_id, "原标题").await;
    let uri = format!("/api/threads/{thread_id}");

    let (status, body) = app
        .put(&uri, Some(&author), json!({ "title": "作者改的" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "作者改的");

    let (status, _) = app
        .put(&uri, Some(&other), json!({ "title": "路人改的" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = app
        .put(&uri, Some(&admin), json!({ "title": "管理员改的" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], "管理员改的");
}

#[tokio::test]
async fn only_admin_can_pin_or_lock() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let author = app.user("author").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (thread_id, _) = app.create_thread(&author, board_id, "话题").await;
    let uri = format!("/api/threads/{thread_id}");

    for req in [json!({ "is_pinned": true }), json!({ "is_locked": true })] {
        let (status, _) = app.put(&uri, Some(&author), req).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    let (status, body) = app
        .put(
            &uri,
            Some(&admin),
            json!({ "is_pinned": true, "is_locked": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["is_pinned"], true);
    assert_eq!(body["is_locked"], true);
    assert_eq!(body["title"], "话题");
}

#[tokio::test]
async fn delete_thread_removes_posts_and_fixes_board_counts() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let author = app.user("author").await;
    let other = app.user("other").await;
    let board_id = app.create_board(&admin, "综合").await;
    let (keep_id, _) = app.create_thread(&author, board_id, "保留").await;
    let (thread_id, _) = app.create_thread(&author, board_id, "删除").await;
    let reply_id = app.reply(&other, thread_id, "回复").await;
    app.reply(&author, thread_id, "再回复").await;

    let board = app.board(board_id).await;
    assert_eq!(board["thread_count"], 2);
    assert_eq!(board["post_count"], 4);

    let uri = format!("/api/threads/{thread_id}");
    let (status, _) = app.delete(&uri, Some(&other)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app.delete(&uri, Some(&author)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&uri, None).await.0, StatusCode::NOT_FOUND);
    let reply_uri = format!("/api/posts/{reply_id}");
    assert_eq!(app.get(&reply_uri, None).await.0, StatusCode::NOT_FOUND);

    let board = app.board(board_id).await;
    assert_eq!(board["thread_count"], 1);
    assert_eq!(board["post_count"], 1);
    app.thread(keep_id).await;

    let (status, _) = app.delete(&uri, Some(&author)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
