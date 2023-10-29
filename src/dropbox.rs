use std::env::var;
use serde::{Deserialize, Serialize};
use reqwest::{header::HeaderMap, Response, Client};
use serde_json::{Value, from_str, from_value, from_reader, json, to_writer_pretty};

#[derive(Serialize, Deserialize)]
struct Token {
    access_token: String,
    token_type: String,
    expires_in: u64
}

#[derive(Deserialize)]
struct ApiResult {
    name: String,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    id: String,
    #[serde(rename = ".tag")]
    tag: String,
    name: String,
    size: Option<u128>,
    #[serde(rename = "path_display")]
    path: String,
    #[serde(rename = "server_modified")]
    modified: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Folder {
    entries: Vec<Value>,
    cursor: String,
    has_more: bool
}

async fn refresh_token() {
    let body = format!("grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
                                var("DROPBOX_REFRESH_TOKEN").expect("Refresh token not set!"), 
                                var("DROPBOX_APP_KEY").expect("Client ID not set!"), 
                                var("DROPBOX_APP_SECRET").expect("Client Secret not set!"));
    let client = Client::new();
    match client.post(var("DROPBOX_TOKEN_URL").expect("Token URL not set!"))
                        .body(body).send().await {
        Ok(res) => {
            match res.status().as_u16() {
                200 => {
                    let json: Token = res.json().await.unwrap();
                    let file = tokio::fs::OpenOptions::new()
                                    .write(true).open("token.json").await.expect("token.json not found!").into_std().await;
                    to_writer_pretty(file, &json).expect("Error in writing to token.json!");
                },
                _ => eprintln!("Error in refreshing token!: {}", res.text().await.unwrap())
            }
        },
        Err(e) => eprintln!("Error in refreshing token!: {}", e),
    };
}

async fn generate_headers() -> HeaderMap {
    let file = tokio::fs::File::open("token.json").await.expect("token.json not found!");
    let token: Token = from_reader(file.into_std().await).expect("Error reading token.json!");
    let mut headers = HeaderMap::new();
    headers.insert("Authorization", format!("Bearer {}", token.access_token).parse().unwrap());
    headers.insert("Content-Type", "application/json".parse().unwrap());
    headers.insert("Dropbox-API-Path-Root", json!({
                                                ".tag": "namespace_id",
                                                "namespace_id": var("DROPBOX_NAMESPACE_ID").unwrap()
                                                }).to_string().parse().unwrap());
    headers.insert("Dropbox-API-Select-User", var("DROPBOX_MEMBER_ID").unwrap().parse().unwrap());
    headers
}

pub async fn list_folder(id: String) -> Result<Value, Value> {
    println!("Listing: {}", id);
    let body = json!({
        "include_deleted": false,
        "include_has_explicit_shared_members": false,
        "include_media_info": false,
        "include_mounted_folders": false,
        "include_non_downloadable_files": false,
        "path": id,
        "recursive": false
    });
    let client = Client::new();
    let res = client.post(var("DROPBOX_LIST_URL").expect("List folder endpoint not set!"))
        .body(body.to_string())
        .headers(generate_headers().await)
        .send().await.unwrap();
    match res.status().as_u16() {
        200 => {
            let folder: Folder = res.json().await.unwrap();
            let mut tree: Vec<Entry> = vec![];
            for entry in folder.entries {
               tree.push(from_value(entry).unwrap());
            };
            Ok(json!({
                "entries": tree,
                "more": folder.has_more,
                "cursor": folder.cursor,
            }))
        },
        401 => {
            refresh_token().await;
            Err(res.json().await.unwrap())
        },
        _ => Err(res.json().await.unwrap()),
    }
}

pub async fn list_folder_continue(cursor: String) -> Result<Value, Value> {
    println!("Continue Listing: {}", cursor);
    let client = Client::new();
    let res = client.post(var("DROPBOX_LIST_CONTINUE_ENDPOINT").expect("List folder continue endpoint not set!"))
        .body(json!({"cursor": cursor}).to_string())
        .headers(generate_headers().await)
        .send().await.unwrap();
    match res.status().as_u16() {
        200 => {
            let folder: Folder = res.json().await.unwrap();
            let mut tree: Vec<Entry> = vec![];
            for entry in folder.entries {
               tree.push(from_value(entry).unwrap());
            };
            Ok(json!({
                "entries": tree,
                "more": folder.has_more,
                "cursor": folder.cursor,
            }))
        },
        401 => {
            refresh_token().await;
            Err(res.json().await.unwrap())
        },
        _ => {
            Err(json!(res.text().await.unwrap()))
        },
    }
}

pub async fn download_file(mut request_headers: HeaderMap, id: String) -> Result<Response, Value> {
    println!("Downloading: {}", id);
    for (k, v) in generate_headers().await {
        request_headers.insert(k.unwrap(), v);
    }
    request_headers.insert("Dropbox-API-Arg",json!({"path": id}).to_string().parse().unwrap());
    request_headers.remove("Content-Type");
    request_headers.remove("Host");
    let client = Client::new();
    let mut res = client.post(var("DROPBOX_DOWNLOAD_ENDPOINT").expect("Download endpoint not set!"))
        .headers(request_headers)
        .send().await.unwrap();
    let api_result: ApiResult = from_str(res.headers().get("Dropbox-API-Result").unwrap().to_str().unwrap()).unwrap();
    res.headers_mut().insert("Content-Disposition", format!("attachment; filename=\"{}\"", api_result.name).parse().unwrap());
    res.headers_mut().remove("Etag");
    res.headers_mut().remove("Dropbox-API-Result");
    res.headers_mut().remove("Content-Security-Policy");
    res.headers_mut().remove("X-Dropbox-Request-Id");
    res.headers_mut().remove("X-Server-Response-Time");
    res.headers_mut().remove("Strict-Transport-Security");
    res.headers_mut().remove("X-Dropbox-Response-Origin");
    match res.status().as_u16() {
        200 => {
            Ok(res)
        },
        401 => {
            refresh_token().await;
            Err(res.json().await.unwrap())
        },
        _ => Err(res.json().await.unwrap()),
    }
}

pub async fn download_zip(mut request_headers: HeaderMap, id: String) -> Result<Response, Value> {
    println!("Zipping: {}", id);
    for (k, v) in generate_headers().await {
        request_headers.insert(k.unwrap(), v);
    }
    request_headers.insert("Dropbox-API-Arg",json!({"path": id}).to_string().parse().unwrap());
    request_headers.remove("Content-Type");
    request_headers.remove("Host");
    let client = Client::new();
    let mut res = client.post(var("DROPBOX_DOWNLOAD_ZIP_ENDPOINT").expect("Download endpoint not set!"))
        .headers(request_headers)
        .send().await.unwrap();
    res.headers_mut().remove("Etag");
    res.headers_mut().remove("Dropbox-API-Result");
    res.headers_mut().remove("Content-Security-Policy");
    res.headers_mut().remove("X-Dropbox-Request-Id");
    res.headers_mut().remove("X-Server-Response-Time");
    res.headers_mut().remove("Strict-Transport-Security");
    res.headers_mut().remove("X-Dropbox-Response-Origin");
    match res.status().as_u16() {
        200 => {
            Ok(res)
        },
        401 => {
            refresh_token().await;
            Err(res.json().await.unwrap())
        },
        _ => Err(res.json().await.unwrap()),
    }
}