use rocket::{get, State, http::Status};
use rocket::serde::json::Json;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use chrono::Utc;

use crate::db::DbPool;
use crate::models::{HistoryLog, AuditLog};

use crate::schema::history_log::dsl::{history_log, created_at as history_created_at};
use crate::schema::audit::dsl::{audit, created_at as audit_created_at};

/// API: GET /api/history
#[get("/api/history")]
pub async fn api_history(pool: &State<DbPool>) -> Result<Json<Vec<HistoryLog>>, Status> {
    let pool = pool.inner().clone();
    let result: Vec<HistoryLog> = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        history_log
            .order(history_created_at.desc())
            .load::<HistoryLog>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(result))
}

/// API: GET /api/audit
#[get("/api/audit")]
pub async fn api_audit(pool: &State<DbPool>) -> Result<Json<Vec<AuditLog>>, Status> {
    let pool = pool.inner().clone();
    let result = rocket::tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        Ok::<_, Status>(get_latest_audit(&mut conn))
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(result))
}

/// Internal helper: fetch latest audit entries
pub fn get_latest_audit(conn: &mut SqliteConnection) -> Vec<AuditLog> {
    audit
        .order(audit_created_at.desc())
        .limit(100)
        .load::<AuditLog>(conn)
        .unwrap_or_default()
}

/// Helper function to log administrative actions to the audit log
pub fn log_audit(
    conn: &mut SqliteConnection,
    actor_val: &str,
    action_type_val: &str,
    target_val: Option<&str>,
    details_val: Option<&str>,
) -> diesel::QueryResult<()> {
    let entry = AuditLog {
        id: 0, // auto-incremented
        actor: actor_val.to_string(),
        action_type: action_type_val.to_string(),
        target: target_val.map(|s| s.to_string()),
        details: details_val.map(|s| s.to_string()),
        created_at: Utc::now().naive_utc(),
    };
    diesel::insert_into(audit)
        .values(&entry)
        .execute(conn)?;
    Ok(())
}
