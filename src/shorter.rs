use color_eyre::Result;

use crate::token::{convert_to_ttl, ttl_expired};

#[derive(Debug, Clone)]
pub struct Shorter {
    pub path: String,
    pub url: String,
    pub ttl: Option<i64>,
}

pub fn locate_shorter(conn: &rusqlite::Connection, path: &str) -> Result<Option<Shorter>> {
    let mut stmt = conn.prepare("SELECT path, url, ttl FROM shorters where path = ?1")?;
    let mut rows = stmt
        .query_map([path], |r| {
            Ok(Shorter {
                path: r.get(0)?,
                url: r.get(1)?,
                ttl: r.get(2)?,
            })
        })?
        .collect::<Result<Vec<Shorter>, _>>()?;

    if rows.len() == 1 && rows[0].path == path {
        return Ok(Some(rows.remove(0)));
    }
    Ok(None)
}

pub fn get_shorter(conn: &rusqlite::Connection, path: &str) -> Result<Option<Shorter>> {
    if let Some(shorter) = locate_shorter(conn, path)? {
        if let Some(ttl) = shorter.ttl {
            if ttl_expired(ttl) {
                Ok(None)
            } else {
                Ok(Some(shorter))
            }
        } else {
            Ok(Some(shorter))
        }
    } else {
        Ok(None)
    }
}

pub fn remove_shorter(conn: &rusqlite::Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM shorters where path = ?1", (path,))?;
    Ok(())
}

pub fn insert_shorter(
    conn: &rusqlite::Connection,
    token: &str,
    path: &str,
    url: &str,
    seconds: Option<i64>,
) -> Result<()> {
    let ttl = seconds.map(|t| convert_to_ttl(t));
    if locate_shorter(conn, path)?.is_some() {
        remove_shorter(conn, path)?;
    }
    conn.execute(
        "INSERT INTO shorters VALUES (NULL, ?1, ?2, ?3, ?4)",
        (path, token, url, ttl),
    )?;
    Ok(())
}
