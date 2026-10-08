use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("未登录或登录已过期")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}不存在")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Db(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("{self:?}");
            "服务器内部错误".to_string()
        } else {
            self.to_string()
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub fn bad_request(msg: impl Into<String>) -> AppError {
    AppError::BadRequest(msg.into())
}

/// 校验去除首尾空白后的字符数在 [min, max] 区间内
pub fn check_len(field: &str, value: &str, min: usize, max: usize) -> AppResult<()> {
    let n = value.trim().chars().count();
    if n < min || n > max {
        return Err(bad_request(format!(
            "{field}长度须在 {min}~{max} 个字符之间"
        )));
    }
    Ok(())
}
