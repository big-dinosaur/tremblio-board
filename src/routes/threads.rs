use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::AnyPool;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::db::{last_insert_id, now};
use crate::error::{AppError, AppResult, check_len};
use crate::models::{PageQuery, Paged, Post, Thread, thread_select};

#[derive(Deserialize)]
pub struct CreateThread {
    title: String,
    /// 首帖内容
    content: String,
}

#[derive(Deserialize)]
pub struct UpdateThread {
    title: Option<String>,
    /// 仅管理员
    is_pinned: Option<bool>,
    /// 仅管理员
    is_locked: Option<bool>,
}

#[derive(Serialize)]
pub struct CreatedThread {
    thread: Thread,
    first_post: Post,
}

pub async fn find(db: &AnyPool, id: i64) -> AppResult<Thread> {
    sqlx::query_as(concat!(thread_select!(), " WHERE t.id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound("话题"))
}

/// 板块下的话题列表：置顶优先，其余按最后回复时间倒序
pub async fn list(
    State(state): State<AppState>,
    Path(board_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<Paged<Thread>>> {
    super::boards::find(&state.db, board_id).await?;
    let (page, per_page, offset) = q.normalize();

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bbs_threads WHERE board_id = ?")
        .bind(board_id)
        .fetch_one(&state.db)
        .await?;
    let items = sqlx::query_as(concat!(
        thread_select!(),
        " WHERE t.board_id = ? \
         ORDER BY t.is_pinned DESC, t.last_post_at DESC, t.id DESC LIMIT ? OFFSET ?"
    ))
    .bind(board_id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(Paged {
        items,
        total,
        page,
        per_page,
    }))
}

/// 发表新话题（同时创建首帖）
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(board_id): Path<i64>,
    Json(req): Json<CreateThread>,
) -> AppResult<(StatusCode, Json<CreatedThread>)> {
    check_len("标题", &req.title, 1, 200)?;
    check_len("内容", &req.content, 1, 100_000)?;
    super::boards::find(&state.db, board_id).await?;

    let ts = now();
    let mut tx = state.db.begin().await?;
    let res = sqlx::query(
        "INSERT INTO bbs_threads (board_id, user_id, title, reply_count, view_count, is_pinned, \
         is_locked, created_at, updated_at, last_post_at) VALUES (?, ?, ?, 0, 0, 0, 0, ?, ?, ?)",
    )
    .bind(board_id)
    .bind(user.0.id)
    .bind(req.title.trim())
    .bind(ts)
    .bind(ts)
    .bind(ts)
    .execute(&mut *tx)
    .await?;
    let thread_id = last_insert_id(&mut tx, &res).await?;

    let res = sqlx::query(
        "INSERT INTO bbs_posts (thread_id, user_id, content, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(thread_id)
    .bind(user.0.id)
    .bind(&req.content)
    .bind(ts)
    .bind(ts)
    .execute(&mut *tx)
    .await?;
    let post_id = last_insert_id(&mut tx, &res).await?;

    sqlx::query(
        "UPDATE bbs_boards SET thread_count = thread_count + 1, post_count = post_count + 1 \
         WHERE id = ?",
    )
    .bind(board_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let thread = find(&state.db, thread_id).await?;
    let first_post = super::posts::find(&state.db, post_id).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedThread { thread, first_post }),
    ))
}

/// 查看话题（浏览数 +1）
pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Thread>> {
    let res = sqlx::query("UPDATE bbs_threads SET view_count = view_count + 1 WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("话题"));
    }
    Ok(Json(find(&state.db, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateThread>,
) -> AppResult<Json<Thread>> {
    let thread = find(&state.db, id).await?;
    user.require_owner(thread.user_id)?;
    if req.is_pinned.is_some() || req.is_locked.is_some() {
        user.require_admin()?;
    }
    let title = req.title.unwrap_or(thread.title);
    check_len("标题", &title, 1, 200)?;
    let is_pinned = req.is_pinned.map_or(thread.is_pinned, i64::from);
    let is_locked = req.is_locked.map_or(thread.is_locked, i64::from);

    sqlx::query(
        "UPDATE bbs_threads SET title = ?, is_pinned = ?, is_locked = ?, updated_at = ? WHERE id = ?",
    )
    .bind(title.trim())
    .bind(is_pinned)
    .bind(is_locked)
    .bind(now())
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(Json(find(&state.db, id).await?))
}

/// 删除话题及其全部帖子，并修正板块计数
pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    let thread = find(&state.db, id).await?;
    user.require_owner(thread.user_id)?;

    let mut tx = state.db.begin().await?;
    let (post_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM bbs_posts WHERE thread_id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query("DELETE FROM bbs_posts WHERE thread_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM bbs_threads WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE bbs_boards SET thread_count = thread_count - 1, post_count = post_count - ? \
         WHERE id = ?",
    )
    .bind(post_count)
    .bind(thread.board_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
