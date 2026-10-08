use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::AnyPool;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::db::{last_insert_id, now};
use crate::error::{AppError, AppResult, bad_request, check_len};
use crate::models::{PageQuery, Paged, Post, post_select};

#[derive(Deserialize)]
pub struct PostContent {
    content: String,
}

pub async fn find(db: &AnyPool, id: i64) -> AppResult<Post> {
    sqlx::query_as(concat!(post_select!(), " WHERE p.id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound("帖子"))
}

/// 话题下的帖子列表（按发帖顺序，第一条为首帖）
pub async fn list(
    State(state): State<AppState>,
    Path(thread_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<Paged<Post>>> {
    super::threads::find(&state.db, thread_id).await?;
    let (page, per_page, offset) = q.normalize();

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bbs_posts WHERE thread_id = ?")
        .bind(thread_id)
        .fetch_one(&state.db)
        .await?;
    let items = sqlx::query_as(concat!(
        post_select!(),
        " WHERE p.thread_id = ? ORDER BY p.id LIMIT ? OFFSET ?"
    ))
    .bind(thread_id)
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

/// 回复话题
pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(thread_id): Path<i64>,
    Json(req): Json<PostContent>,
) -> AppResult<(StatusCode, Json<Post>)> {
    check_len("内容", &req.content, 1, 100_000)?;
    let thread = super::threads::find(&state.db, thread_id).await?;
    if thread.is_locked != 0 && !user.is_admin() {
        return Err(AppError::Forbidden("话题已锁定，无法回复".into()));
    }

    let ts = now();
    let mut tx = state.db.begin().await?;
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
        "UPDATE bbs_threads SET reply_count = reply_count + 1, last_post_at = ? WHERE id = ?",
    )
    .bind(ts)
    .bind(thread_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE bbs_boards SET post_count = post_count + 1 WHERE id = ?")
        .bind(thread.board_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(find(&state.db, post_id).await?)))
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Post>> {
    Ok(Json(find(&state.db, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(req): Json<PostContent>,
) -> AppResult<Json<Post>> {
    check_len("内容", &req.content, 1, 100_000)?;
    let post = find(&state.db, id).await?;
    user.require_owner(post.user_id)?;

    sqlx::query("UPDATE bbs_posts SET content = ?, updated_at = ? WHERE id = ?")
        .bind(&req.content)
        .bind(now())
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(find(&state.db, id).await?))
}

/// 删除回复；首帖不能单独删除，需删除整个话题
pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    let post = find(&state.db, id).await?;
    user.require_owner(post.user_id)?;
    let thread = super::threads::find(&state.db, post.thread_id).await?;

    let (first_id,): (i64,) = sqlx::query_as("SELECT MIN(id) FROM bbs_posts WHERE thread_id = ?")
        .bind(post.thread_id)
        .fetch_one(&state.db)
        .await?;
    if first_id == id {
        return Err(bad_request("首帖不能单独删除，请删除整个话题"));
    }

    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM bbs_posts WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let (last_post_at,): (i64,) =
        sqlx::query_as("SELECT MAX(created_at) FROM bbs_posts WHERE thread_id = ?")
            .bind(post.thread_id)
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "UPDATE bbs_threads SET reply_count = reply_count - 1, last_post_at = ? WHERE id = ?",
    )
    .bind(last_post_at)
    .bind(post.thread_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE bbs_boards SET post_count = post_count - 1 WHERE id = ?")
        .bind(thread.board_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
