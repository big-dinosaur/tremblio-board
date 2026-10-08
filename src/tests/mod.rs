//! HTTP 层集成测试：每个测试使用独立的临时 SQLite 文件，走与生产相同的连接与迁移流程，
//! 通过 `tower::ServiceExt::oneshot` 直接把请求交给路由，无需监听端口。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::AnyPool;
use tower::ServiceExt;

use crate::AppState;
use crate::config::Config;

mod boards;
mod posts;
mod threads;
mod users;

pub const PASSWORD: &str = "secret123";

pub struct TestApp {
    app: Router,
    pub db: AnyPool,
    db_path: PathBuf,
}

impl TestApp {
    pub async fn new() -> Self {
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        let db_path = std::env::temp_dir().join(format!(
            "tremblio-board-test-{}-{}.db",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let config = Config {
            database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
            bind_addr: String::new(),
            token_ttl: 3600,
        };
        let db = crate::db::connect(&config.database_url)
            .await
            .expect("连接测试数据库失败");
        let app = crate::routes::router().with_state(AppState {
            db: db.clone(),
            config: Arc::new(config),
        });
        Self { app, db, db_path }
    }

    /// 发送请求，返回状态码和 JSON 响应体（无响应体时为 `Value::Null`）
    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let req = match body {
            Some(body) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();

        let resp = self.app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("响应体不是合法 JSON")
        };
        (status, json)
    }

    pub async fn get(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.call(Method::GET, uri, token, None).await
    }

    pub async fn post(&self, uri: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.call(Method::POST, uri, token, Some(body)).await
    }

    pub async fn put(&self, uri: &str, token: Option<&str>, body: Value) -> (StatusCode, Value) {
        self.call(Method::PUT, uri, token, Some(body)).await
    }

    pub async fn delete(&self, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
        self.call(Method::DELETE, uri, token, None).await
    }

    pub async fn register(&self, username: &str) -> Value {
        let (status, body) = self
            .post(
                "/api/auth/register",
                None,
                json!({ "username": username, "password": PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "注册失败: {body}");
        body
    }

    pub async fn login(&self, username: &str) -> String {
        let (status, body) = self
            .post(
                "/api/auth/login",
                None,
                json!({ "username": username, "password": PASSWORD }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "登录失败: {body}");
        body["token"].as_str().unwrap().to_string()
    }

    /// 注册并登录，返回 token。第一个调用的用户会成为管理员。
    pub async fn user(&self, username: &str) -> String {
        self.register(username).await;
        self.login(username).await
    }

    pub async fn create_board(&self, token: &str, name: &str) -> i64 {
        let (status, body) = self
            .post("/api/boards", Some(token), json!({ "name": name }))
            .await;
        assert_eq!(status, StatusCode::CREATED, "创建板块失败: {body}");
        body["id"].as_i64().unwrap()
    }

    /// 返回 (话题 id, 首帖 id)
    pub async fn create_thread(&self, token: &str, board_id: i64, title: &str) -> (i64, i64) {
        let (status, body) = self
            .post(
                &format!("/api/boards/{board_id}/threads"),
                Some(token),
                json!({ "title": title, "content": format!("{title} 的首帖") }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "发帖失败: {body}");
        (
            body["thread"]["id"].as_i64().unwrap(),
            body["first_post"]["id"].as_i64().unwrap(),
        )
    }

    pub async fn reply(&self, token: &str, thread_id: i64, content: &str) -> i64 {
        let (status, body) = self
            .post(
                &format!("/api/threads/{thread_id}/posts"),
                Some(token),
                json!({ "content": content }),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "回帖失败: {body}");
        body["id"].as_i64().unwrap()
    }

    pub async fn board(&self, id: i64) -> Value {
        let (status, body) = self.get(&format!("/api/boards/{id}"), None).await;
        assert_eq!(status, StatusCode::OK, "获取板块失败: {body}");
        body
    }

    /// 注意：查看话题会使浏览数 +1
    pub async fn thread(&self, id: i64) -> Value {
        let (status, body) = self.get(&format!("/api/threads/{id}"), None).await;
        assert_eq!(status, StatusCode::OK, "获取话题失败: {body}");
        body
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.db_path.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}
