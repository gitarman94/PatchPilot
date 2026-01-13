use rocket::{get, post, delete, State};
use rocket::serde::json::Json;
use rocket::form::Form;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::{Utc, Duration};

use crate::auth::AuthUser;
use crate::db::{
    DbPool,
    fetch_action_ttl as db_fetch_action_ttl,
    update_action_ttl as db_update_action_ttl,
    insert_history as db_insert_history,
    load_settings as db_load_settings,
};
use crate::models::{Action, NewAction, NewActionTarget};
use crate::schema::{actions, action_targets};

use crate::db::ServerSettingsRow;

#[derive(FromForm)]
pub struct SubmitActionForm {
    pub command: String,
    pub target_device_id: i64,
    pub ttl_seconds: Option<i64>,
}

#[post("/api/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Form<SubmitActionForm>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, Status> {
    // Separate clones for DB and audit
    let pool_for_db = pool.inner().clone();
    let username = user.username.clone();
    let form = form.into_inner();

    // Insert action + target + history in a blocking task
    let action_id_res = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;

        let now = Utc::now().naive_utc();

        // Load server settings for TTL default/cap
        let settings_row: ServerSettingsRow =
            db_load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;

        // Determine TTL
        let ttl_to_use = form
            .ttl_seconds
            .map(|t| t.min(settings_row.default_action_ttl_seconds))
            .unwrap_or(settings_row.default_action_ttl_seconds);

        let expires_at = now + Duration::seconds(ttl_to_use);

        // Insert action
        let new_action = NewAction {
            action_type: form.command.clone(),
            parameters: None,
            author: Some(username.clone()),
            created_at: now,
            expires_at,
            canceled: false,
        };

        diesel::insert_into(actions::table)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|e| {
                log::error!("Failed to insert action: {:?}", e);
                Status::InternalServerError
            })?;

        // Get action id (last_insert_rowid via SQLite) — use `.order(id.desc()).select(id).first(...)`
        let last_id: i64 = actions::table
            .select(actions::id)
            .order(actions::id.desc())
            .first::<i64>(&mut conn)
            .map_err(|e| {
                log::error!("Failed to obtain action id: {:?}", e);
                Status::InternalServerError
            })?;

        // Insert action target
        let new_target = NewActionTarget::pending(last_id, form.target_device_id);
        diesel::insert_into(action_targets::table)
            .values(&new_target)
            .execute(&mut conn)
            .map_err(|e| {
                log::error!("Failed to insert action target: {:?}", e);
                Status::InternalServerError
            })?;

        // Optionally insert a history entry
        let _ = db_insert_history(
            &mut conn,
            &format!("action_submitted:{}", last_id),
            Some(&form.target_device_id.to_string()),
            Some(&username),
        );

        Ok(last_id)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(serde_json::json!({
        "action_id": action_id_res,
        "status": "queued"
    })))
}

#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();
    let res = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<Action>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let rows = actions::table.load::<Action>(&mut conn).map_err(|_| Status::InternalServerError)?;
        Ok(rows)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(res))
}
