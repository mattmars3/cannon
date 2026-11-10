use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use cannon::*;
use futures_util::TryStreamExt;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use rusqlite::{Connection, OptionalExtension};
use std::{collections::HashMap, path::PathBuf};
use std::{env, sync::Arc};
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

type UploadResult<T> = Result<T, (StatusCode, &'static str)>;

struct CannonServerState {
    conn: rusqlite::Connection,
    api_key: String,
}

impl CannonServerState {
    pub fn new() -> Arc<Mutex<CannonServerState>> {
        let conn = Connection::open(CANNON_DB_PATH).expect("Unable to open database path");
        let api_key = env::var("CANNON_API_KEY")
            .expect("Must specify CANNON_API_KEY as an environment variable");
        Arc::new(Mutex::new(Self { conn, api_key }))
    }
}

#[tokio::main]
async fn main() {
    create_db_file_if_not_exists();
    let cannon_server_state = CannonServerState::new();
    create_table_if_not_exists(cannon_server_state.clone()).await;

    let app = Router::new()
        .route("/{uuid}", get(get_file))
        .route("/", put(upload_file))
        .with_state(cannon_server_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Unable to listen on specified port.");

    println!("Running cannon server on :3000");

    axum::serve(listener, app)
        .await
        .expect("Unable to start server.");
}

async fn user_is_authenticated(state: Arc<Mutex<CannonServerState>>, headers: &HeaderMap) -> bool {
    let Ok(provided_api_key) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(false)
    else {
        return false;
    };

    // TODO: Confirm this is absolutely secure
    state.lock().await.api_key == provided_api_key
}

async fn upload_file(
    State(state): State<Arc<Mutex<CannonServerState>>>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    body: Body,
) -> UploadResult<impl IntoResponse> {
    // check the API key here
    if !user_is_authenticated(state.clone(), &headers).await {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    let delete_on_download: bool = match params.get(DELETE_ON_DOWNLOAD_QUERY_PARAM_NAME) {
        Some(delete_on_download) => {
            let Ok(delete_on_download) = delete_on_download.parse::<bool>() else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Failed to parse delete_on_download query parameter",
                ));
            };
            delete_on_download
        }
        None => DELETE_ON_DOWNLOAD_DEFAULT,
    };

    let (uuid, bytes) = match stream_to_local_file(body).await {
        Ok((uuid, bytes)) => (uuid, bytes),
        Err((status, err_msg)) => {
            let generic_err_msg = "Unable to stream payload to local file";
            eprintln!("{err_msg}");
            return Err((status, generic_err_msg));
        }
    };

    let filename: &String = params.get(FILENAME_QUERY_PARAM_NAME).unwrap_or(&uuid);

    println!("Received file <{filename}> of {bytes} bytes");

    // TODO: turn this into a struct so that the different metadata parameters are better organized
    if let Err(e) = store_file_record_in_db(state, &uuid, filename, delete_on_download).await {
        eprintln!("{e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to store record of file in database",
        ));
    }

    return Ok(uuid);
}

async fn stream_to_local_file(body: Body) -> UploadResult<(String, u64)> {
    let uuid = Uuid::new_v4().to_string();
    let temp_path = PathBuf::from(CANNON_DATA_PATH).join(format!("{uuid}.tmp"));
    let final_path = PathBuf::from(CANNON_DATA_PATH).join(&uuid);

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .await
        .map_err(|e| {
            eprintln!("{e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create upload file",
            )
        })?;

    let stream = body.into_data_stream().map_err(tokio::io::Error::other);
    // take one more byte than the max so we can test later
    let mut reader = StreamReader::new(stream).take(MAX_UPLOAD_BYTES + 1);

    let written = match tokio::io::copy(&mut reader, &mut file).await {
        Ok(n) => n,
        Err(e) => {
            eprintln!("{e}");
            drop(file);
            let _ = fs::remove_file(&temp_path).await;
            return Err((StatusCode::BAD_REQUEST, "Upload failed"));
        }
    };

    if written > MAX_UPLOAD_BYTES {
        eprintln!("File exceeds upload limits");
        drop(file);
        let _ = fs::remove_file(&temp_path).await;
        return Err((StatusCode::PAYLOAD_TOO_LARGE, "File is too large"));
    }

    if file.flush().await.is_err() {
        drop(file);
        let _ = fs::remove_file(&temp_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not finish writing upload",
        ));
    }
    drop(file);

    if fs::rename(&temp_path, &final_path).await.is_err() {
        let _ = fs::remove_file(&temp_path).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not finalize upload",
        ));
    }

    Ok((uuid, written))
}

async fn store_file_record_in_db(
    state: Arc<Mutex<CannonServerState>>,
    uuid: &str,
    filename: &str,
    delete_on_download: bool,
) -> rusqlite::Result<usize> {
    let now = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
    let guard = state.lock().await;

    guard.conn.execute(
        "INSERT INTO cannon (uuid, filename, uploaded_at, delete_on_download) VALUES (?1, ?2, ?3, ?4)",
        (uuid, filename, now, delete_on_download),
    )
}

fn create_db_file_if_not_exists() {
    let p = PathBuf::from(CANNON_DB_PATH);
    if !p.exists() {
        println!("Database file does not exist. Creating {}", CANNON_DB_PATH);
        std::fs::File::create(p).expect("Unable to create database file");
    }
}

async fn create_table_if_not_exists(state: Arc<Mutex<CannonServerState>>) {
    let guard = state.lock().await;
    guard
        .conn
        .execute(
            "
CREATE TABLE IF NOT EXISTS cannon (
id INTEGER PRIMARY KEY,
uuid TEXT NOT NULL,
filename TEXT NOT NULL,
uploaded_at TEXT NOT NULL,
delete_on_download INTEGER NOT NULL CHECK (delete_on_download IN (0, 1))
)",
            (),
        )
        .expect("Unable to run initial database check");
}

async fn get_file(
    State(state): State<Arc<Mutex<CannonServerState>>>,
    Path(uuid): Path<String>,
) -> Result<Response, (StatusCode, &'static str)> {
    let row = {
        let state = state.lock().await;

        state
            .conn
            .query_row(
                "SELECT uuid, filename, delete_on_download
                 FROM cannon
                 WHERE uuid = ?1",
                [uuid.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
    };

    // TODO: add a method to delete the file after the response has been served
    let Some((uuid, filename, _delete_on_download)) = row else {
        return Err((StatusCode::NOT_FOUND, "File not found"));
    };

    // stream the file to the user
    let file_path = PathBuf::from(CANNON_DATA_PATH).join(&uuid);
    let file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "File not found but should exist"))?;

    let encoded_filename = utf8_percent_encode(&filename, NON_ALPHANUMERIC);
    let disposition = format!("attachment; filename*=UTF-8''{encoded_filename}");

    let mut response = Response::new(Body::from_stream(ReaderStream::new(file)));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Invalid filename"))?,
    );

    Ok(response)
}
