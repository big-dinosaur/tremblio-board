use serde::{Deserialize, Serialize, Serializer};
use sqlx::FromRow;

/// 数据库中布尔值以整数存储（Any 驱动无法在两种数据库间统一解码 bool），输出时转成 JSON 布尔。
fn int_as_bool<S: Serializer>(v: &i64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_bool(*v != 0)
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip)]
    pub password_hash: String,
    pub nickname: String,
    #[serde(serialize_with = "int_as_bool")]
    pub is_admin: i64,
    #[serde(serialize_with = "int_as_bool")]
    pub is_banned: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_login_at: i64,
}

/// sqlx 0.9 只接受静态 SQL，用宏配合 `concat!` 在编译期拼接，避免运行时 format!
macro_rules! user_columns {
    () => {
        "id, username, password_hash, nickname, is_admin, is_banned, \
         created_at, updated_at, last_login_at"
    };
}
pub(crate) use user_columns;

#[derive(Debug, FromRow, Serialize)]
pub struct Board {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub sort_order: i64,
    pub thread_count: i64,
    pub post_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

macro_rules! board_columns {
    () => {
        "id, name, description, sort_order, thread_count, post_count, created_at, updated_at"
    };
}
pub(crate) use board_columns;

#[derive(Debug, FromRow, Serialize)]
pub struct Thread {
    pub id: i64,
    pub board_id: i64,
    pub user_id: i64,
    pub author: String,
    pub title: String,
    pub reply_count: i64,
    pub view_count: i64,
    #[serde(serialize_with = "int_as_bool")]
    pub is_pinned: i64,
    #[serde(serialize_with = "int_as_bool")]
    pub is_locked: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_post_at: i64,
}

/// 已包含 FROM/JOIN，调用方只需追加 WHERE/ORDER BY
macro_rules! thread_select {
    () => {
        "SELECT t.id, t.board_id, t.user_id, u.username AS author, t.title, \
     t.reply_count, t.view_count, t.is_pinned, t.is_locked, t.created_at, t.updated_at, \
     t.last_post_at FROM bbs_threads t JOIN bbs_users u ON u.id = t.user_id"
    };
}
pub(crate) use thread_select;

#[derive(Debug, FromRow, Serialize)]
pub struct Post {
    pub id: i64,
    pub thread_id: i64,
    pub user_id: i64,
    pub author: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

macro_rules! post_select {
    () => {
        "SELECT p.id, p.thread_id, p.user_id, u.username AS author, \
     p.content, p.created_at, p.updated_at FROM bbs_posts p JOIN bbs_users u ON u.id = p.user_id"
    };
}
pub(crate) use post_select;

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PageQuery {
    /// 返回 (page, per_page, offset)
    pub fn normalize(&self) -> (i64, i64, i64) {
        let page = self.page.unwrap_or(1).max(1);
        let per_page = self.per_page.unwrap_or(20).clamp(1, 100);
        (page, per_page, (page - 1) * per_page)
    }
}

#[derive(Debug, Serialize)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}
