use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sqlx::{sqlite::SqliteQueryResult, Pool, Sqlite};

use crate::{auth::Auth, AppState};

const DEFAULT_AVATAR: &[u8] = include_bytes!("avatar_default.jpeg");

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct CreateUserBody {
    username: String,
    password: String,
}

pub async fn create(
    State(app): State<Arc<AppState>>,
    Json(body): Json<CreateUserBody>,
) -> Response {
    if app.find_user(&body.username).await.is_ok() {
        return StatusCode::CONFLICT.into_response();
    }

    if let Err(e) = app.create_user(&body.username, &body.password).await {
        eprintln!("{e:?}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match app.find_user(&body.username).await {
        Ok(user) => Json(user).into_response(),
        Err(e) => {
            eprintln!("{e:?}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn get(
    Auth(_): Auth,
    Path(username): Path<String>,
    State(app): State<Arc<AppState>>,
) -> Response {
    let Ok(user) = app.find_user(&username).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    Json(user).into_response()
}

pub async fn get_me(Auth(user): Auth) -> Json<User> {
    Json(user)
}

pub async fn avatar(Path(username): Path<String>, State(app): State<Arc<AppState>>) -> Response {
    let Ok(Some(data)) = app.get_user_avatar(&username).await else {
        return DEFAULT_AVATAR.into_response();
    };

    data.into_response()
}

impl AppState {
    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SqliteQueryResult, sqlx::Error> {
        sqlx::query("INSERT INTO users (username, password) VALUES (?, ?)")
            .bind(username)
            .bind(password)
            .execute(&self.db)
            .await
    }

    pub async fn find_user(&self, username: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(&self.db)
            .await
    }

    pub async fn get_user_avatar(&self, username: &str) -> Result<Option<Vec<u8>>, sqlx::Error> {
        let data = sqlx::query_as::<_, (Vec<u8>,)>(
            "
            SELECT data FROM avatars
            WHERE user_id = (
                SELECT id FROM users WHERE username = ?
            )",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await;
        data.map(|x| x.map(|(data,)| data))
    }
}

pub async fn create_table(pool: &Pool<Sqlite>) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "
    CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        username TEXT,
        password TEXT
    );
    
    CREATE TABLE IF NOT EXISTS avatars (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER,
        data BLOB,
        FOREIGN KEY (user_id) REFERENCES users (id)
    );",
    )
    .execute(pool)
    .await
}
