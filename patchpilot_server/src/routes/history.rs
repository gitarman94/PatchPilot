use diesel::prelude::*;
use rocket::{get, State};
use rocket::serde::json::Json;
use rocket::http::Status;

<<<<<<< HEAD
use crate::DbPool;
use crate::models::HistoryRecord;
use crate::schema::history_log;
=======
use crate::db::pool::DbPool;
use crate::models::AuditLog;
use crate::schema::audit_log::dsl::*;
>>>>>>> 2a90c386955747fed0a4ffc63e4416f8a242b3f2

#[get("/api/history")]
pub async fn api_history(
    pool: &State<DbPool>,
) -> Result<Json<serde_json::Value>, Status> {
    let mut conn = pool
        .get()
        .map_err(|_| Status::InternalServerError)?;

<<<<<<< HEAD
    let rows = history_log::table
        .order(history_log::created_at.desc())
=======
    let rows = audit_log
        .order(created_at.desc())
>>>>>>> 2a90c386955747fed0a4ffc63e4416f8a242b3f2
        .limit(500)
<<<<<<< HEAD
        .load::<HistoryRecord>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
=======
        .load::<AuditLog>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
>>>>>>> 2a90c386955747fed0a4ffc63e4416f8a242b3f2

    Ok(Json(serde_json::json!({
        "history": rows
    })))
}
