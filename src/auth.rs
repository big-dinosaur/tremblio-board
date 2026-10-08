use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::db::now;
use crate::error::{AppError, AppResult};
use crate::models::User;

pub fn hash_password(password: &str) -> AppResult<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("密码哈希失败: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 生成随机 token（返回给客户端的明文）
pub fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::fill(&mut buf);
    to_hex(&buf)
}

/// 数据库中只保存 token 的 SHA-256 摘要
pub fn hash_token(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

/// 已登录用户提取器：从 `Authorization: Bearer <token>` 解析并校验。
pub struct CurrentUser(pub User);

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.0.is_admin != 0
    }

    pub fn require_admin(&self) -> AppResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden("需要管理员权限".into()))
        }
    }

    /// 本人或管理员
    pub fn require_owner(&self, owner_id: i64) -> AppResult<()> {
        if self.0.id == owner_id || self.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden("没有权限操作该内容".into()))
        }
    }
}

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> AppResult<Self> {
        let token = bearer_token(&parts.headers).ok_or(AppError::Unauthorized)?;
        let token_hash = hash_token(token);
        let ts = now();

        let user: User = sqlx::query_as(
            "SELECT u.id, u.username, u.password_hash, u.nickname, u.is_admin, u.is_banned, \
                    u.created_at, u.updated_at, u.last_login_at \
             FROM bbs_tokens t JOIN bbs_users u ON u.id = t.user_id \
             WHERE t.token_hash = ? AND t.expires_at > ?",
        )
        .bind(&token_hash)
        .bind(ts)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

        if user.is_banned != 0 {
            return Err(AppError::Forbidden("账号已被封禁".into()));
        }

        sqlx::query("UPDATE bbs_tokens SET last_used_at = ? WHERE token_hash = ?")
            .bind(ts)
            .bind(&token_hash)
            .execute(&state.db)
            .await?;

        Ok(CurrentUser(user))
    }
}
