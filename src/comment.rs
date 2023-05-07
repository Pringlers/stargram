use std::sync::Arc;

use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use sqlx::{sqlite::SqliteQueryResult, types::chrono, Pool, Sqlite};

use crate::{auth::Auth, AppState};

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct Comment {
    pub id: i64,
    pub feed_id: String,
    pub username: String,
    pub content: String,
    #[serde(with = "::chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct NewComment {
    pub content: String,
}

pub async fn get_comments(
    Auth(_): Auth,
    State(state): State<Arc<AppState>>,
    Path(feed_id): Path<String>,
) -> Response {
    let comments = match state.get_comments(&feed_id).await {
        Ok(comments) => comments,
        Err(e) => {
            tracing::error!("{e:?}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(comments).into_response()
}

pub async fn create_comment(
    Auth(user): Auth,
    State(state): State<Arc<AppState>>,
    Path(feed_id): Path<String>,
    Json(NewComment { content }): Json<NewComment>,
) -> Response {
    let comment = match state.create_comment(&feed_id, user.id, &content).await {
        Ok(comment) => comment,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    Json(comment).into_response()
}

impl AppState {
    async fn get_comments(&self, feed_id: &str) -> Result<Vec<Comment>, sqlx::Error> {
        sqlx::query_as(
            "SELECT comments.id, comments.feed_id, comments.content, comments.created_at, users.username FROM comments
            INNER JOIN users ON comments.user_id = users.id
            WHERE comments.feed_id = ?
            ORDER BY comments.created_at ASC",
        )
        .bind(feed_id)
        .fetch_all(&self.db)
        .await
    }

    async fn create_comment(
        &self,
        feed_id: &str,
        user_id: i64,
        content: &str,
    ) -> Result<Comment, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO comments (feed_id, user_id, content) VALUES (?, ?, ?);
            SELECT comments.id, comments.feed_id, comments.content, comments.created_at, users.username FROM comments
            INNER JOIN users ON comments.user_id = users.id
            WHERE comments.id = last_insert_rowid()",
        )
        .bind(feed_id)
        .bind(user_id)
        .bind(content)
        .fetch_one(&self.db)
        .await
    }
}

pub async fn create_table(pool: &Pool<Sqlite>) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            feed_id TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (feed_id) REFERENCES feeds (id),
            FOREIGN KEY (user_id) REFERENCES users (id)
        );",
    )
    .execute(pool)
    .await
}
