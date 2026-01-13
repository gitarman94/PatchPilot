use rocket::{get, post, State};
use rocket::serde::json::Json;
use rocket::form::Form;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::{Utc, Duration};

use crate::auth::AuthUser;
use crate::db::{DbPool, log_audit as db_log_audit, load_settings as db_load_settings};
use crate::models::{Action, NewAction, NewActionTarget};
use crate::schema::{actions, action_targets};

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
    let pool_for_db = pool.inner().clone();
    let username = user.username.clone();
    let form = form.into_inner();

    let action_id_res = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;

        let now = Utc::now().naive_utc();

        // Load server settings for TTL default/cap
        let settings_row = db_load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;

        // Determine TTL
        let ttl_to_use = form
            .ttl_seconds
            .map(|t| if t < settings_row.default_action_ttl_seconds { t } else { settings_row.default_action_ttl_seconds })
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

        // Retrieve action id
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

        // Audit the action submission
        let _ = db_log_audit(
            &mut conn,
            &username,
            &format!("action_submitted:{}", last_id),
            Some(&form.target_device_id.to_string()),
            Some("action submitted"),
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

use rocket::Route;
pub fn routes() -> Vec<Route> {
    routes![submit_action, list_actions]
}
