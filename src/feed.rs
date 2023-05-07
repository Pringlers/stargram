use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use image::ImageFormat;
use sqlx::{sqlite::SqliteQueryResult, types::chrono, Pool, Sqlite};
use uuid::Uuid;

use crate::{auth::Auth, AppState};

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct Feed {
    pub id: String,
    pub user_id: i64,
    pub caption: String,
    pub image_count: u8,
    #[serde(with = "::chrono::serde::ts_milliseconds")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct FeedWithUser {
    pub id: String,
    pub username: String,
    pub caption: String,
    pub image_count: u8,
    pub created_at: String,
}

const ALLOWED_FORMATS: &[ImageFormat] = &[ImageFormat::Png, ImageFormat::Jpeg];

pub async fn create(
    Auth(user): Auth,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Response {
    let feed_id = Uuid::new_v4().to_string();
    let mut caption = None;
    let mut count = 0;
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) if field.name() == Some("caption") => {
                let Ok(text) = field.text().await else { return StatusCode::BAD_REQUEST.into_response() };
                caption = Some(text);
                continue;
            }
            Ok(Some(field)) => {
                let Ok(buf) = field.bytes().await else { return StatusCode::BAD_REQUEST.into_response() };

                let Ok(format) = image::guess_format(&buf) else {
                    return StatusCode::BAD_REQUEST.into_response();
                };

                if !ALLOWED_FORMATS.contains(&format) {
                    return StatusCode::BAD_REQUEST.into_response();
                }

                if let Err(e) = state.create_image(&feed_id, count, &buf).await {
                    eprintln!("Failed to store image: {e}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }

                count += 1;
                continue;
            }
            Ok(None) if count == 0 => return StatusCode::BAD_REQUEST.into_response(),
            Ok(None) => {
                let Some(caption) = caption else { return StatusCode::BAD_REQUEST.into_response() };
                match state.create_feed(&feed_id, user.id, &caption, count).await {
                    Ok(feed) => return Json(feed).into_response(),
                    Err(e) => {
                        eprintln!("Failed to create feed: {e}");
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                };
            }
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
    }
}

pub async fn get_feeds(Auth(_): Auth, State(state): State<Arc<AppState>>) -> Response {
    let Ok(feeds) = state.get_feeds().await else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    Json(feeds).into_response()
}

pub async fn get_feed_image(
    Path((feed_id, index)): Path<(String, u8)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let Ok(data) = state.get_image(&feed_id, index).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    data.into_response()
}

impl AppState {
    async fn create_feed(
        &self,
        feed_id: &str,
        user_id: i64,
        caption: &str,
        image_count: u8,
    ) -> Result<Feed, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO feeds (id, user_id, caption, image_count) VALUES (?, ?, ?, ?);
            SELECT * FROM feeds WHERE id = ?",
        )
        .bind(feed_id)
        .bind(user_id)
        .bind(caption)
        .bind(image_count)
        .bind(feed_id)
        .fetch_one(&self.db)
        .await
    }

    async fn get_feeds(&self) -> Result<Vec<FeedWithUser>, sqlx::Error> {
        sqlx::query_as(
            "SELECT feeds.id, feeds.caption, feeds.image_count, feeds.created_at, users.username FROM feeds
            INNER JOIN users ON feeds.user_id = users.id
            ORDER BY feeds.created_at DESC",
        ).fetch_all(&self.db).await
    }

    async fn create_image(
        &self,
        feed_id: &str,
        index: u8,
        data: &[u8],
    ) -> Result<SqliteQueryResult, sqlx::Error> {
        sqlx::query("INSERT INTO images (feed_id, position, data) VALUES (?, ?, ?)")
            .bind(feed_id)
            .bind(index)
            .bind(data)
            .execute(&self.db)
            .await
    }

    async fn get_image(&self, feed_id: &str, index: u8) -> Result<Vec<u8>, sqlx::Error> {
        sqlx::query_scalar("SELECT data FROM images WHERE feed_id = ? AND position = ?")
            .bind(feed_id)
            .bind(index)
            .fetch_one(&self.db)
            .await
    }
}

pub async fn create_table(pool: &Pool<Sqlite>) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS feeds (
            id TEXT PRIMARY KEY,
            user_id INTEGER NOT NULL,
            caption TEXT NOT NULL,
            image_count INTEGER NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users (id)
        );

        CREATE TABLE IF NOT EXISTS images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            feed_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            data BLOB NOT NULL
        );",
    )
    .execute(pool)
    .await
}
