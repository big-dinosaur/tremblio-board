use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// 登录 token 有效期（秒）
    pub token_ttl: i64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://bbs.db?mode=rwc".to_string());
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
        let token_ttl_hours: i64 = env::var("TOKEN_TTL_HOURS")
            .unwrap_or_else(|_| "168".to_string())
            .parse()
            .context("TOKEN_TTL_HOURS 必须是整数")?;
        Ok(Self {
            database_url,
            bind_addr,
            token_ttl: token_ttl_hours * 3600,
        })
    }
}
