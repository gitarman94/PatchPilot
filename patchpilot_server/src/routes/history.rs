use diesel::prelude::*;
use rocket::{get, State};
use rocket::serde::json::Json;

use crate::db::pool::DbPool;
use crate::models::HistoryRecord;
use crate::schema::history_log::dsl::*;

#[get("/history")]
pub async fn get_history(pool: &State<DbPool>)
    -> Result<Json<Vec<HistoryRecord>>, String>
{
    let pool = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|e| e.to_string())?;
        history_log
            .order(created_at.desc())
            .limit(500)
            .load::<HistoryRecord>(&mut conn)
            .map(Json)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[get("/api/history")]
pub async fn api_history(pool: &State<DbPool>)
    -> Result<Json<serde_json::Value>, rocket::http::Status>
{
    let mut conn = pool.get()
        .map_err(|_| rocket::http::Status::InternalServerError)?;

    let rows = history_log
        .order(created_at.desc())
        .limit(500)
        .load::<HistoryRecord>(&mut conn)
        .map_err(|_| rocket::http::Status::InternalServerError)?;

    Ok(Json(serde_json::json!({ "history": rows })))
}
