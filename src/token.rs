use chrono::{DateTime, TimeDelta, Utc};
use color_eyre::Result;
use log::warn;

#[derive(Debug, Clone)]
pub struct Token {
    pub token: String,
    pub ttl: Option<i64>,
}

pub fn ttl_expired(ttl: i64) -> bool {
    if let Some(ddl) = DateTime::<Utc>::from_timestamp_micros(ttl) {
        let now = Utc::now();
        if now >= ddl {
            return true;
        } else {
            return false;
        }
    } else {
        warn!("Invalid timestamp in tokens {}?!", ttl);
        return true;
    }
}

pub fn convert_to_ttl(seconds: i64) -> i64 {
    let now = Utc::now();
    let delta = TimeDelta::seconds(seconds);

    (now + delta).timestamp_micros()
}

pub fn add_token(conn: &rusqlite::Connection, token: &str, seconds: Option<i64>) -> Result<()> {
    let ttl = seconds.map(|t| convert_to_ttl(t));
    if locate_token(conn, token)?.is_some() {
        remove_token(conn, token)?;
    }
    conn.execute("INSERT INTO tokens VALUES (NULL, ?1, ?2)", (token, ttl))?;
    Ok(())
}

pub fn remove_token(conn: &rusqlite::Connection, token: &str) -> Result<()> {
    conn.execute("DELETE FROM tokens where token = ?1", (token,))?;
    Ok(())
}

pub fn locate_token(conn: &rusqlite::Connection, token: &str) -> Result<Option<Token>> {
    let mut stmt = conn.prepare("SELECT token, ttl FROM tokens where token = ?1")?;
    let mut rows = stmt
        .query_map([token], |r| {
            Ok(Token {
                token: r.get(0)?,
                ttl: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<Token>, _>>()?;

    if rows.len() == 1 && rows[0].token == token {
        return Ok(Some(rows.remove(0)));
    }
    Ok(None)
}

pub fn token_allowed(conn: &rusqlite::Connection, token: &str) -> Result<bool> {
    if let Some(token) = locate_token(conn, token)? {
        if let Some(ttl) = token.ttl {
            Ok(!ttl_expired(ttl))
        } else {
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

pub fn list_token(conn: &rusqlite::Connection) -> Result<Vec<Token>> {
    let mut stmt = conn.prepare("SELECT token, ttl FROM tokens")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Token {
                token: r.get(0)?,
                ttl: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<Token>, _>>()?;

    Ok(rows)
}
