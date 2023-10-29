use axum::{routing::get, Router};

mod dropbox;
mod handler;

#[tokio::main]
async fn main() {
    dotenvy::from_path_override(std::path::Path::new("config.env")).expect("config.env not found!");

    let app = Router::new()
                    .route("/list_folder/:id", get(handler::list))
                    .route("/list_folder_continue/:cursor", get(handler::list_continue))
                    .route("/download_file/:id", get(handler::download))
                    .route("/download_folder/:id", get(handler::zip));

    let address = format!("0.0.0.0:{}", std::env::var("DROPBOX_BACKEND_PORT").unwrap());

    axum::Server::bind(&address.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

}