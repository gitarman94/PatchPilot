use diesel::prelude::*;
use rocket::{get, State};
use rocket::serde::json::Json;
use rocket::http::Status;

use crate::DbPool;
use crate::models::HistoryRecord;
use crate::schema::history_log::dsl::*;

#[get("/api/history")]
pub async fn api_history(pool: &State<DbPool>) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    let rows = history_log
        .order(created_at.desc())
        .limit(500)
        .load::<HistoryRecord>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(serde_json::json!({
        "history": rows
    })))
}
