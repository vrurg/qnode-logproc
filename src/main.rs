mod app;
mod parser;
mod reader;
mod stats;
mod types;

#[tokio::main]
async fn main() {
    app::App::run().await.expect("Application must not fail");
}
