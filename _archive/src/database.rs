use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::config::PostgresConfig;

#[derive(Debug, Clone)]
pub struct DatabaseReadiness {
    pool: PgPool,
}

impl DatabaseReadiness {
    pub fn new(config: &PostgresConfig) -> Self {
        let options = config
            .connect_options()
            .expect("Postgres config should be validated before readiness setup");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy_with(options);

        Self { pool }
    }

    pub async fn check(&self) -> Result<(), sqlx::Error> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
    }
}
