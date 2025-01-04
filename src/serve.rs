use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::HOST, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, put},
    Form, Router,
};
use chrono::{DateTime, Utc};
use clap::Args;
use color_eyre::Result;
use log::{debug, info, warn};
use rand::distributions::DistString;
use serde::Deserialize;
use tokio::{
    net::TcpListener,
    sync::{Mutex, RwLock},
};
use url::Url;

use crate::{
    open_db,
    shorter::{get_shorter, insert_shorter},
    token::token_allowed,
};

#[derive(Debug, Clone, Args)]
pub struct ServeCommand {
    #[arg(short, long, env = "DATABASE_PATH")]
    pub db: PathBuf,

    #[arg(short, long, default_value = "0.0.0.0:1566")]
    pub listen: String,
}

#[derive(Debug, Clone)]
struct ServeState {
    db: Arc<Mutex<rusqlite::Connection>>,
}

async fn index() -> Result<Response, StatusCode> {
    Ok(Html(include_str!("index.html")).into_response())
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
    headers
        .get(HOST)
        .map(|t| {
            let e = t.to_str().ok();
            e.map(|h| url::Url::parse(&format!("https://{}", h)).ok())
        })
        .ok_or(StatusCode::NOT_FOUND)?
        .flatten()
        .ok_or(StatusCode::BAD_REQUEST)
}

fn put_shorter_inner(
    db: &rusqlite::Connection,
    host: Url,
    shorter: ShorterArguments,
) -> Result<Response, StatusCode> {
    let path = if let Some(path) = shorter.path {
        if path.len() == 0 {
            random_path(8)
        } else {
            path.clone()
        }
    } else {
        random_path(8)
    };
    let allowed = check_token(&db, &shorter.token)?;
    let url = urlencoding::decode(&shorter.url)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .to_string();

    if url.len() == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if allowed {
        if let Err(e) = insert_shorter(&db, &shorter.token, &path, &url, shorter.seconds) {
            warn!("Fail to insert shorter due to {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            let redirect = host
                .join(&path)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Body::new(redirect.to_string()).into_response())
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Deserialize)]
struct ShorterArguments {
    token: String,
    path: Option<String>,
    url: String,
    #[serde(alias = "ttl")]
    seconds: Option<i64>,
}

async fn put_shorter(
    headers: HeaderMap,
    Query(params): Query<ShorterArguments>,
    State(state): State<ServeState>,
) -> Result<Response, StatusCode> {
    let host = extract_host(&headers)?;

    let db = state.db.lock().await;
    put_shorter_inner(&db, host, params)
}

async fn post_shorter(
    headers: HeaderMap,
    State(state): State<ServeState>,
    Form(params): Form<ShorterArguments>,
) -> Result<Response, StatusCode> {
    let host = extract_host(&headers)?;
    let db = state.db.lock().await;
    put_shorter_inner(&db, host, params)
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
    State(state): State<ServeState>,
) -> Result<Response, StatusCode> {
    let db = state.db.lock().await;

    debug!("Getting a path: {}", path);
    if let Some(shorter) = get_shorter(&db, &path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        Ok(Redirect::to(&shorter.url).into_response())
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn serve_main(args: ServeCommand) -> Result<()> {
    let db = open_db(&args.db)?;
    let app = Router::new()
        .route("/", get(index))
        .route("/api", put(put_shorter).post(post_shorter))
        .route("/{*path}", get(handler))
        .with_state(ServeState {
            db: Arc::new(Mutex::new(db)),
        });

    info!("Serving at {}", &args.listen);
    let listener = TcpListener::bind(args.listen).await.unwrap();

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    Ok(())
}
