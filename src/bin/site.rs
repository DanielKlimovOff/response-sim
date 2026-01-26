use response_sim::*;
use sqlx::SqlitePool;
use axum::{response::Html, routing::get, Router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_pool = SqlitePool::connect(&std::env::var("DATABASE_URL")?).await?;
    let mut card_base = CardBase::new(db_pool).await;

    let app = Router::new().route("/", get(handler)).route("/bye", get(handler2));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn handler() -> Html<&'static str> {
    Html("<h1>Hello, World!</h1>")
}

async fn handler2() -> Html<&'static str> {
    Html("<h1>Bye, World!</h1>")
}