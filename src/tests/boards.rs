use axum::http::StatusCode;
use serde_json::json;

use super::TestApp;

#[tokio::test]
async fn create_board_requires_admin() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let member = app.user("member").await;
    let req = json!({ "name": "灌水区" });

    let (status, _) = app.post("/api/boards", None, req.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app.post("/api/boards", Some(&member), req.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = app.post("/api/boards", Some(&admin), req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "灌水区");
    assert_eq!(body["description"], "");
    assert_eq!(body["thread_count"], 0);
    assert_eq!(body["post_count"], 0);
}

#[tokio::test]
async fn create_board_validates_name() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    for name in ["   ".to_string(), "长".repeat(65)] {
        let (status, _) = app
            .post("/api/boards", Some(&admin), json!({ "name": name }))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn list_boards_sorted_by_sort_order_then_id() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    for (name, sort_order) in [("B", 2), ("A", 1), ("C", 1)] {
        let (status, _) = app
            .post(
                "/api/boards",
                Some(&admin),
                json!({ "name": name, "sort_order": sort_order }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let (status, body) = app.get("/api/boards", None).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<_> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["A", "C", "B"]);
}

#[tokio::test]
async fn get_missing_board_returns_404() {
    let app = TestApp::new().await;
    let (status, body) = app.get("/api/boards/9999", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "板块不存在");
}

#[tokio::test]
async fn update_board_changes_only_given_fields() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let member = app.user("member").await;
    let id = app.create_board(&admin, "旧名称").await;
    let uri = format!("/api/boards/{id}");

    let (status, _) = app
        .put(&uri, Some(&member), json!({ "name": "新名称" }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, body) = app
        .put(&uri, Some(&admin), json!({ "description": "新描述" }))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["name"], "旧名称");
    assert_eq!(body["description"], "新描述");

    let (status, _) = app.put("/api/boards/9999", Some(&admin), json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delete_board_cascades_to_threads_and_posts() {
    let app = TestApp::new().await;
    let admin = app.user("admin").await;
    let member = app.user("member").await;
    let board_id = app.create_board(&admin, "待删除").await;
    let (thread_id, post_id) = app.create_thread(&member, board_id, "话题").await;
    let uri = format!("/api/boards/{board_id}");

    let (status, _) = app.delete(&uri, Some(&member)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app.delete(&uri, Some(&admin)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(app.get(&uri, None).await.0, StatusCode::NOT_FOUND);
    let thread_uri = format!("/api/threads/{thread_id}");
    assert_eq!(app.get(&thread_uri, None).await.0, StatusCode::NOT_FOUND);
    let post_uri = format!("/api/posts/{post_id}");
    assert_eq!(app.get(&post_uri, None).await.0, StatusCode::NOT_FOUND);

    let (status, _) = app.delete(&uri, Some(&admin)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
