use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;

mod auth;
mod user;

pub struct AppState {
    pub db: sqlx::Pool<sqlx::Sqlite>,
}

impl AppState {
    pub fn new(db: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { db }
    }
}

#[tokio::main]
async fn main() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite:stargram.db")
        .await
        .unwrap();

    user::create_table(&pool).await.unwrap();
    auth::create_table(&pool).await.unwrap();

    let app = Arc::new(AppState::new(pool));
    let router = Router::new()
        .route("/users", post(user::create))
        .route("/users/@me", get(user::get_me))
        .route("/user/:name", get(user::get))
        .with_state(app);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("Running on {addr:?}");
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}
