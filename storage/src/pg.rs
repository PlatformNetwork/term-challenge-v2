use std::fmt;

use deadpool_postgres::{Config, CreatePoolError, ManagerConfig, Pool, RecyclingMethod, Runtime};
use serde::Deserialize;
use tokio_postgres::NoTls;

pub type PgPool = Pool;

#[derive(Clone, Deserialize)]
pub struct PgConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
}

impl fmt::Debug for PgConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PgConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &"[REDACTED]")
            .field("dbname", &self.dbname)
            .field("pool_size", &self.pool_size)
            .finish()
    }
}

fn default_pool_size() -> usize {
    16
}

impl Default for PgConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            user: "postgres".to_string(),
            password: "postgres".to_string(),
            dbname: "term_challenge".to_string(),
            pool_size: default_pool_size(),
        }
    }
}

pub fn create_pool(cfg: &PgConfig) -> Result<PgPool, CreatePoolError> {
    let mut config = Config::new();
    config.host = Some(cfg.host.clone());
    config.port = Some(cfg.port);
    config.user = Some(cfg.user.clone());
    config.password = Some(cfg.password.clone());
    config.dbname = Some(cfg.dbname.clone());
    config.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    config.create_pool(Some(Runtime::Tokio1), NoTls)
}
