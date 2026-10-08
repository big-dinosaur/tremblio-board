use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::{
    CurrentUser, bearer_token, generate_token, hash_password, hash_token, verify_password,
};
use crate::db::{last_insert_id, now};
use crate::error::{AppError, AppResult, bad_request, check_len};
use crate::models::{User, user_columns};

#[derive(Deserialize)]
pub struct RegisterReq {
    username: String,
    password: String,
    nickname: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResp {
    token: String,
    expires_at: i64,
    user: User,
}

async fn find_user_by_name(state: &AppState, username: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM bbs_users WHERE username = ?"
    ))
    .bind(username)
    .fetch_optional(&state.db)
    .await?)
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> AppResult<(StatusCode, Json<User>)> {
    let username = req.username.trim();
    check_len("用户名", username, 3, 32)?;
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(bad_request("用户名只能包含字母、数字和下划线"));
    }
    if req.password.chars().count() < 6 || req.password.len() > 128 {
        return Err(bad_request("密码长度须在 6~128 之间"));
    }
    let nickname = req.nickname.as_deref().unwrap_or(username).trim();
    check_len("昵称", nickname, 1, 64)?;

    if find_user_by_name(&state, username).await?.is_some() {
        return Err(AppError::Conflict("用户名已被占用".into()));
    }

    let password_hash = hash_password(&req.password)?;
    let ts = now();
    let mut tx = state.db.begin().await?;
    // 第一个注册的用户自动成为管理员
    let (user_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM bbs_users")
        .fetch_one(&mut *tx)
        .await?;
    let res = sqlx::query(
        "INSERT INTO bbs_users (username, password_hash, nickname, is_admin, is_banned, \
         created_at, updated_at, last_login_at) VALUES (?, ?, ?, ?, 0, ?, ?, 0)",
    )
    .bind(username)
    .bind(&password_hash)
    .bind(nickname)
    .bind(if user_count == 0 { 1_i64 } else { 0 })
    .bind(ts)
    .bind(ts)
    .execute(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("用户名已被占用".into())
        }
        _ => e.into(),
    })?;
    let id = last_insert_id(&mut tx, &res).await?;
    let user: User = sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM bbs_users WHERE id = ?"
    ))
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> AppResult<Json<LoginResp>> {
    let user = find_user_by_name(&state, req.username.trim())
        .await?
        .filter(|u| verify_password(&req.password, &u.password_hash))
        .ok_or_else(|| bad_request("用户名或密码错误"))?;
    if user.is_banned != 0 {
        return Err(AppError::Forbidden("账号已被封禁".into()));
    }

    let ts = now();
    let token = generate_token();
    let expires_at = ts + state.config.token_ttl;

    // 顺便清理该用户已过期的 token
    sqlx::query("DELETE FROM bbs_tokens WHERE user_id = ? AND expires_at <= ?")
        .bind(user.id)
        .bind(ts)
        .execute(&state.db)
        .await?;
    sqlx::query(
        "INSERT INTO bbs_tokens (user_id, token_hash, created_at, expires_at, last_used_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(hash_token(&token))
    .bind(ts)
    .bind(expires_at)
    .bind(ts)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE bbs_users SET last_login_at = ? WHERE id = ?")
        .bind(ts)
        .bind(user.id)
        .execute(&state.db)
        .await?;

    let user = User {
        last_login_at: ts,
        ..user
    };
    Ok(Json(LoginResp {
        token,
        expires_at,
        user,
    }))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<StatusCode> {
    let token = bearer_token(&headers).ok_or(AppError::Unauthorized)?;
    sqlx::query("DELETE FROM bbs_tokens WHERE token_hash = ?")
        .bind(hash_token(token))
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(user: CurrentUser) -> Json<User> {
    Json(user.0)
}
