use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use color_eyre::Result;
use itertools::Itertools;
use log::warn;
use serve::{serve_main, ServeCommand};
use token::{add_token, list_token, locate_token, remove_token, ttl_expired};

mod shorter;
mod token;
mod serve;

#[derive(Debug, Clone, Parser)]
struct LinkShorterCli {
    #[command(subcommand)]
    command: ShorterCommand,
}

#[derive(Subcommand, Debug, Clone)]
enum ShorterCommand {
    Serve(ServeCommand),
    #[command(name = "add-token")]
    AddToken(AddTokenComamnd),
    #[command(name = "remove-token")]
    RemoveToken(RemoveComamnd),
    #[command(name = "list-token")]
    ListToken(ListTokenComamnd),
}



#[derive(Debug, Clone, Args)]
struct AddTokenComamnd {
    #[arg(short, long, env = "DATABASE_PATH")]
    pub db: PathBuf,

    #[arg(short, long)]
    pub token: String,

    #[arg(short, long)]
    pub seconds: Option<i64>,
}

#[derive(Debug, Clone, Args)]
struct RemoveComamnd {
    #[arg(short, long, env = "DATABASE_PATH")]
    pub db: PathBuf,

    #[arg(short, long)]
    pub token: String,
}

#[derive(Debug, Clone, Args)]
struct ListTokenComamnd {
    #[arg(short, long, env = "DATABASE_PATH")]
    pub db: PathBuf,
}

fn create_table(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS shorters (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT NOT NULL UNIQUE,
        url TEXT NOT NULL,
        ttl INTEGER
    );",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tokens (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        token TEXT NOT NULL UNIQUE,
        ttl INTEGER
    );",
        (),
    )?;
    Ok(())
}

fn open_db(db: &Path) -> Result<rusqlite::Connection> {
    let db = rusqlite::Connection::open(db)?;
    create_table(&db)?;

    Ok(db)
}

async fn link_shorter(args: LinkShorterCli) -> Result<()> {
    match args.command {
        ShorterCommand::AddToken(args) => {
            let db: rusqlite::Connection = open_db(&args.db)?;
            add_token(&db, &args.token, args.seconds)?;
            println!("Token added");
        }
        ShorterCommand::ListToken(args) => {
            let db = open_db(&args.db)?;
            let tokens = list_token(&db)?;

            println!("Token\tTTL");
            for token in tokens {
                println!(
                    "{}\t{}",
                    token.token,
                    if let Some(ttl) = token.ttl {
                        let dt = DateTime::<Utc>::from_timestamp_micros(ttl).unwrap();
                        if ttl_expired(ttl) {
                            format!("{} (expired)", dt)
                        } else {
                            format!("{}", dt)
                        }
                    } else {
                        "-".to_string()
                    }
                );
            }
        }
        ShorterCommand::RemoveToken(args) => {
            let db = open_db(&args.db)?;
            if locate_token(&db, &args.token)?.is_some() {
                remove_token(&db, &args.token)?;
                println!("Token removed");
            } else {
                println!("No such token");
            }
        }
        ShorterCommand::Serve(args) => {
            serve_main(args).await?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    color_eyre::install().expect("Fail to install color_eyre");
    if let Ok(dot_file) = std::env::var("DOT") {
        dotenvy::from_path(dot_file)?;
    } else {
        // Allows failure
        let _ = dotenvy::dotenv();
    }
    env_logger::init();

    let args = LinkShorterCli::parse();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(link_shorter(args))
}
