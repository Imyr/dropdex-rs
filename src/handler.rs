use axum::{Json, headers::HeaderMap, extract::Path, body::StreamBody, response::IntoResponse};

use crate::dropbox::{list_folder, download_file, download_zip, list_folder_continue};

pub async fn list(Path(id): Path<String>) -> impl IntoResponse {
    match list_folder(id).await {
        Ok(body) => Json(body),
        Err(e) => Json(e),
    }
}

pub async fn list_continue(Path(cursor): Path<String>) -> impl IntoResponse {
    match list_folder_continue(cursor).await {
        Ok(body) => Json(body),
        Err(e) => Json(e),
    }
}

pub async fn download(headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    match download_file(headers, id).await {
        Ok(response) => (response.status(), response.headers().to_owned(), StreamBody::from(response.bytes_stream())).into_response(),
        Err(e) => Json(e).into_response(),
    } 
}

pub async fn zip(headers: HeaderMap, Path(id): Path<String>) -> impl IntoResponse {
    match download_zip(headers, id).await {
        Ok(response) => (response.status(), response.headers().to_owned(), StreamBody::from(response.bytes_stream())).into_response(),
        Err(e) => Json(e).into_response(),
    } 
}