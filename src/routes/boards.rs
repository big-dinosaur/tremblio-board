use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::AnyPool;

use crate::AppState;
use crate::auth::CurrentUser;
use crate::db::{last_insert_id, now};
use crate::error::{AppError, AppResult, check_len};
use crate::models::{Board, board_columns};

#[derive(Deserialize)]
pub struct CreateBoard {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    sort_order: i64,
}

#[derive(Deserialize)]
pub struct UpdateBoard {
    name: Option<String>,
    description: Option<String>,
    sort_order: Option<i64>,
}

pub async fn find(db: &AnyPool, id: i64) -> AppResult<Board> {
    sqlx::query_as(concat!(
        "SELECT ",
        board_columns!(),
        " FROM bbs_boards WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound("板块"))
}

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Board>>> {
    let boards = sqlx::query_as(concat!(
        "SELECT ",
        board_columns!(),
        " FROM bbs_boards ORDER BY sort_order, id"
    ))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(boards))
}

pub async fn get(State(state): State<AppState>, Path(id): Path<i64>) -> AppResult<Json<Board>> {
    Ok(Json(find(&state.db, id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<CreateBoard>,
) -> AppResult<(StatusCode, Json<Board>)> {
    user.require_admin()?;
    check_len("板块名称", &req.name, 1, 64)?;
    check_len("板块描述", &req.description, 0, 500)?;

    let ts = now();
    let mut conn = state.db.acquire().await?;
    let res = sqlx::query(
        "INSERT INTO bbs_boards (name, description, sort_order, thread_count, post_count, \
         created_at, updated_at) VALUES (?, ?, ?, 0, 0, ?, ?)",
    )
    .bind(req.name.trim())
    .bind(req.description.trim())
    .bind(req.sort_order)
    .bind(ts)
    .bind(ts)
    .execute(&mut *conn)
    .await?;
    let id = last_insert_id(&mut conn, &res).await?;
    drop(conn);
    Ok((StatusCode::CREATED, Json(find(&state.db, id).await?)))
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateBoard>,
) -> AppResult<Json<Board>> {
    user.require_admin()?;
    let board = find(&state.db, id).await?;
    let name = req.name.unwrap_or(board.name);
    let description = req.description.unwrap_or(board.description);
    check_len("板块名称", &name, 1, 64)?;
    check_len("板块描述", &description, 0, 500)?;

    sqlx::query(
        "UPDATE bbs_boards SET name = ?, description = ?, sort_order = ?, updated_at = ? WHERE id = ?",
    )
    .bind(name.trim())
    .bind(description.trim())
    .bind(req.sort_order.unwrap_or(board.sort_order))
    .bind(now())
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(Json(find(&state.db, id).await?))
}

/// 删除板块会级联删除其下所有话题和帖子
pub async fn delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> AppResult<StatusCode> {
    user.require_admin()?;
    let res = sqlx::query("DELETE FROM bbs_boards WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("板块"));
    }
    Ok(StatusCode::NO_CONTENT)
}
