use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use sqlx::{sqlite::SqliteQueryResult, Pool, Sqlite};
use uuid::Uuid;

use crate::{user::User, AppState};

pub struct Auth(pub User);

#[async_trait::async_trait]
impl FromRequestParts<Arc<AppState>> for Auth {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let Some(session) = parts
            .headers
            .get("Authentication")
            .and_then(|value| value.to_str().ok())
        else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        match state.resolve_session(session).await {
            Ok(user) => Ok(Auth(user)),
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

impl AppState {
    pub async fn create_session(&self, user: &User) -> Result<String, sqlx::Error> {
        let key = Uuid::new_v4().to_string();

        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user.id)
            .execute(&self.db)
            .await?;

        sqlx::query("INSERT INTO sessions (user_id, key) VALUES (?, ?)")
            .bind(user.id)
            .bind(&key)
            .execute(&self.db)
            .await?;

        Ok(key)
    }

    pub async fn resolve_session(&self, key: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as(
            "SELECT * FROM users WHERE id = (SELECT user_id FROM sessions WHERE key = ?)",
        )
        .bind(key)
        .fetch_one(&self.db)
        .await
    }
}

pub async fn create_table(pool: &Pool<Sqlite>) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER,
        key TEXT,
        FOREIGN KEY (user_id) REFERENCES users(id)
    );",
    )
    .execute(pool)
    .await
}
