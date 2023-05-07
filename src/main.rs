use std::net::SocketAddr;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::filter::LevelFilter;

mod auth;
mod channel;
mod comment;
mod feed;
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
    tracing_subscriber::fmt::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let pool = SqlitePoolOptions::new()
        .connect("sqlite:stargram.db")
        .await
        .unwrap();

    user::create_table(&pool).await.unwrap();
    auth::create_table(&pool).await.unwrap();
    feed::create_table(&pool).await.unwrap();
    comment::create_table(&pool).await.unwrap();

    let cors_layer = CorsLayer::permissive();
    let app_state = Arc::new(AppState::new(pool));

    let user_router = Router::new()
        .route("/", post(user::create))
        .route("/@me", get(user::get_me))
        .route("/:name", get(user::get))
        .route("/avatar/:name", get(user::avatar));

    let channel_router = Router::new()
        .route("/", post(channel::create))
        .route("/preview", get(channel::preview))
        .route("/:id/messages", get(channel::get_messages))
        .route("/:id/messages", post(channel::create_message));

    let feed_router = Router::new()
        .route("/", get(feed::get_feeds))
        .route("/", post(feed::create))
        .route("/:id/comments", get(comment::get_comments))
        .route("/:id/comments", post(comment::create_comment))
        .route("/:id/img/:index", get(feed::get_feed_image));

    let router = Router::new()
        .route("/login", post(auth::login))
        .nest("/users", user_router)
        .nest("/channels", channel_router)
        .nest("/feeds", feed_router)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer)
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("Running on {addr:?}");
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}
