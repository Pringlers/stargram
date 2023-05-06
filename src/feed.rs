use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use image::ImageFormat;
use sqlx::{sqlite::SqliteQueryResult, Pool, Sqlite};
use uuid::Uuid;

use crate::{auth::Auth, AppState};

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct Feed {
    pub id: i64,
    pub feed_id: String,
    pub user_id: i64,
    pub caption: String,
    pub image_count: u8,
    pub created_at: String,
}

const ALLOWED_FORMATS: &[ImageFormat] = &[ImageFormat::Png, ImageFormat::Jpeg];

fn store_image(feed_id: &str, index: u8, buf: &[u8]) -> anyhow::Result<()> {
    let Ok(format) = image::guess_format(buf) else {
        anyhow::bail!("Cannot parse format");
    };
    if !ALLOWED_FORMATS.contains(&format) {
        anyhow::bail!("Unsupported format");
    }

    Ok(std::fs::write(
        format!("images/{feed_id}_{index}.png"),
        buf,
    )?)
}

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

                if let Err(e) = store_image(&feed_id, count, &buf) {
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

pub async fn get_home() {}

pub async fn get_user_feeds() {}

pub async fn get_comments() {}

pub async fn create_comment() {}

impl AppState {
    async fn create_feed(
        &self,
        feed_id: &str,
        user_id: i64,
        caption: &str,
        image_count: u8,
    ) -> Result<Feed, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO feeds (feed_id, user_id, caption, image_count) VALUES (?, ?, ?, ?);
            SELECT * FROM feeds WHERE feed_id = ?",
        )
        .bind(feed_id)
        .bind(user_id)
        .bind(caption)
        .bind(image_count)
        .bind(feed_id)
        .fetch_one(&self.db)
        .await
    }
}

pub async fn create_table(pool: &Pool<Sqlite>) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS feeds (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            feed_id TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            caption TEXT NOT NULL,
            image_count INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )",
    )
    .execute(pool)
    .await
}
