use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{body::Body, extract::{Path, Query, State}, http::{header::HOST, HeaderMap, HeaderValue, StatusCode}, response::{IntoResponse, Redirect, Response}, routing::{get, put}, Router};
use chrono::{DateTime, Utc};
use clap::Args;
use color_eyre::Result;
use log::{debug, info, warn};
use rand::distributions::DistString;
use tokio::{net::TcpListener, sync::{Mutex, RwLock}};
use url::Url;

use crate::{open_db, shorter::{get_shorter, insert_shorter}, token::token_allowed};

#[derive(Debug, Clone, Args)]
pub struct ServeCommand {
    #[arg(short, long, env = "DATABASE_PATH")]
    pub db: PathBuf,

    #[arg(short, long, default_value = "0.0.0.0:1566")]
    pub listen: String
}

#[derive(Debug, Clone)]
struct ServeState {
    db: Arc<Mutex<rusqlite::Connection>>
}

async fn index() -> &'static str {
    include_str!("index.txt")
}

fn check_token(db: &rusqlite::Connection, token: &str) -> Result<bool, StatusCode> {
    match token_allowed(db, token) {
        Ok(t) => Ok(t),
        Err(e) => {
            warn!("Fail to check token validity due to {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn random_path(length: usize) -> String {
    rand::distributions::Alphanumeric.sample_string(&mut rand::thread_rng(), length)
}

fn extract_host(headers: &HeaderMap) -> Result<Url, StatusCode> {
    headers.get(HOST).map(|t| {
        let e = t.to_str().ok();
        e.map(|h| url::Url::parse(&format!("http://{}", h)).ok())
    }).ok_or(StatusCode::NOT_FOUND)?.flatten().ok_or(StatusCode::FORBIDDEN)
}

async fn put_shorter(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<ServeState>
) -> Result<Response, StatusCode> {
    let host = extract_host(&headers)?;
    let token = params.get("token").ok_or(StatusCode::UNAUTHORIZED)?;
    let path = if let Some(path) = params.get("path") {
        path.clone()
    } else {
        random_path(8)
    };
    let url = urlencoding::decode(params.get("url").ok_or(StatusCode::NOT_FOUND)?).map_err(|_| StatusCode::FORBIDDEN)?.to_string();
    let seconds = if let Some(seconds) =  params.get("ttl").or(params.get("seconds")) {
        Some(i64::from_str_radix(&seconds, 10).map_err(|e| StatusCode::FORBIDDEN)?)
    } else {
        None
    };
    let db = state.db.lock().await;
    let allowed = check_token(&db, token)?;

    if allowed {
        if let Err(e) = insert_shorter(&db, &path, &url, seconds) {
            warn!("Fail to insert shorter due to {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            let redirect = host.join(&path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Body::new(redirect.to_string()).into_response())
        }
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn handler(
    Path(path): Path<String>,
    State(state): State<ServeState>
) -> Result<Response, StatusCode> {
    let db = state.db.lock().await;

    debug!("Getting a path: {}", path);
    if let Some(shorter) = get_shorter(&db, &path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        Ok(Redirect::to(&shorter.url).into_response())
    } else {
        Ok(Body::new("No such shorter".to_string()).into_response())
    }
}

pub async fn serve_main(args: ServeCommand) -> Result<()> {
    let db = open_db(&args.db)?;
    let app = Router::new()
        .route("/", get(index))
        .route("/api", put(put_shorter))
        .route("/{*path}", get(handler))
        .with_state(ServeState {db: Arc::new(Mutex::new(db))});

    info!("Serving at {}", &args.listen);
    let listener = TcpListener::bind(args.listen).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    Ok(())
}