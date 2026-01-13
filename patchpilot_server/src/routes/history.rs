use rocket::{get, Route, State, http::Status};
use rocket::serde::json::Json;
use diesel::prelude::*;

use crate::db::{DbPool, log_audit as db_log_audit};
use crate::models::{HistoryEntry, AuditLog};
use crate::schema::history_log::dsl::{history_log, created_at as history_created_at};
use crate::schema::audit::dsl::{audit, created_at as audit_created_at};

/// API: GET /api/history
#[get("/api/history")]
pub async fn api_history(pool: &State<DbPool>) -> Result<Json<Vec<HistoryEntry>>, Status> {
    let pool_clone = pool.inner().clone();
    let result: Vec<HistoryEntry> = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        history_log
            .order(history_created_at.desc())
            .load::<HistoryEntry>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

/// API: GET /api/audit
#[get("/api/audit")]
pub async fn api_audit(pool: &State<DbPool>) -> Result<Json<Vec<AuditLog>>, Status> {
    let pool_clone = pool.inner().clone();
    let result: Vec<AuditLog> = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool_clone.get().map_err(|_| Status::InternalServerError)?;
        audit
            .order(audit_created_at.desc())
            .limit(100)
            .load::<AuditLog>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(result))
}

/// Route list for history APIs
pub fn api_routes() -> Vec<Route> {
    routes![api_history, api_audit]
}
