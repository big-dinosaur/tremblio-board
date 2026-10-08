use anyhow::{Result, bail};
use sqlx::any::AnyQueryResult;
use sqlx::any::{AnyPoolOptions, install_default_drivers};
use sqlx::migrate::Migrator;
use sqlx::{AnyConnection, AnyPool};

static MYSQL_MIGRATOR: Migrator = sqlx::migrate!("./migrations/mysql");
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("./migrations/sqlite");

/// 根据 URL 前缀连接 MySQL 或 SQLite，并执行对应方言的迁移脚本。
pub async fn connect(database_url: &str) -> Result<AnyPool> {
    install_default_drivers();

    let migrator = if database_url.starts_with("mysql:") || database_url.starts_with("mariadb:") {
        &MYSQL_MIGRATOR
    } else if database_url.starts_with("sqlite:") {
        &SQLITE_MIGRATOR
    } else {
        bail!("不支持的 DATABASE_URL，仅支持 mysql:// 或 sqlite://");
    };

    let pool = AnyPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    migrator.run(&pool).await?;
    Ok(pool)
}

/// 获取刚插入行的自增主键。
/// MySQL 驱动会在结果里带回该值；SQLite 在 Any 驱动下不提供，需在同一连接上查询 last_insert_rowid()。
pub async fn last_insert_id(conn: &mut AnyConnection, res: &AnyQueryResult) -> sqlx::Result<i64> {
    if let Some(id) = res.last_insert_id() {
        return Ok(id);
    }
    let (id,): (i64,) = sqlx::query_as("SELECT last_insert_rowid()")
        .fetch_one(conn)
        .await?;
    Ok(id)
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间早于 1970")
        .as_secs() as i64
}
