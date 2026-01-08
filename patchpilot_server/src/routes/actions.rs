// src/routes/actions.rs
use rocket::{get, post, delete, State};
use rocket::serde::json::Json;
use rocket::form::Form;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::{Utc, Duration};
use crate::auth::AuthUser;
use crate::db::{DbPool, update_action_ttl as db_update_action_ttl, fetch_action_ttl as db_fetch_action_ttl, insert_history as db_insert_history};
use crate::models::{Action, NewAction, NewActionTarget};
use crate::schema::{actions, action_targets};
use crate::db::ServerSettingsRow; // for TTL update

#[derive(FromForm)]
pub struct SubmitActionForm {
    pub command: String,
    pub target_device_id: i64,
    pub ttl_seconds: Option<i64>,
}

/// Submit a new action
#[post("/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Form<SubmitActionForm>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool_inner = pool.inner().clone();
    let form = form.into_inner();
    let username = user.username.clone();

    // Spawn blocking to insert action + target + history
    let inserted_action_id: i64 = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let ttl = form.ttl_seconds.unwrap_or(3600);
        let now = Utc::now().naive_utc();
        let expires_at = now + Duration::seconds(ttl);

        let new_action = NewAction {
            action_type: form.command,
            parameters: None,
            author: Some(username.clone()),
            created_at: now,
            expires_at,
            canceled: false,
        };

        diesel::insert_into(actions::table)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        let action_id: i64 = actions::table
            .select(actions::id)
            .order(actions::id.desc())
            .first(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        let target = NewActionTarget::pending(action_id, form.target_device_id);
        diesel::insert_into(action_targets::table)
            .values(&target)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        // Add a history record for this action submission
        let history = crate::db::NewHistory {
            action_id,
            device_name: None,
            actor: Some(&username),
            action_type: "action.submit",
            details: Some(&format!("Target device: {}", form.target_device_id)),
            created_at: now,
        };
        let _ = db_insert_history(&mut conn, &history).map_err(|_| Status::InternalServerError)?;

        Ok(action_id)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    // Log audit asynchronously via the routes/history::log_audit helper (keeps single async interface)
    // This helper spawns a blocking task internally and returns a Result<(), Status>.
    let _ = crate::routes::history::log_audit(
        &pool.inner(),
        &username,
        "action.submit",
        Some(&inserted_action_id.to_string()),
        Some(&format!("Target device: {}", form.target_device_id)),
    ).await;

    Ok(Status::Created)
}

/// List all actions
#[get("/actions")]
pub async fn list_actions(
    pool: &State<DbPool>,
    _user: AuthUser,
) -> Result<Json<Vec<Action>>, Status> {
    let pool_inner = pool.inner().clone();
    let result = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<Action>, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let all_actions = actions::table
            .order(actions::created_at.desc())
            .load::<Action>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(all_actions)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(result))
}

/// Cancel an action
#[post("/actions/cancel/<action_id_param>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id_param: i64,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool_inner = pool.inner().clone();
    let username = user.username.clone();

    let res = rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let now = Utc::now().naive_utc();
        diesel::update(actions::table.filter(actions::id.eq(action_id_param)))
            .set(actions::expires_at.eq(now))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        // Insert history record
        let history = crate::db::NewHistory {
            action_id: action_id_param,
            device_name: None,
            actor: Some(&username),
            action_type: "action.cancel",
            details: None,
            created_at: now,
        };
        let _ = db_insert_history(&mut conn, &history).map_err(|_| Status::InternalServerError)?;
        Ok(())
    }).await.map_err(|_| Status::InternalServerError)??;

    // Async audit log
    let _ = crate::routes::history::log_audit(&pool.inner(), &username, "action.cancel", Some(&action_id_param.to_string()), None).await;

    Ok(Status::Ok)
}

/// List targets for a specific action
#[get("/actions/targets/<action_id_param>")]
pub async fn list_action_targets(
    pool: &State<DbPool>,
    action_id_param: i64,
) -> Result<Json<Vec<(i64, String, Option<String>)>>, Status> {
    let pool_inner = pool.inner().clone();
    let targets = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<(i64, String, Option<String>)>, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let results = action_targets::table
            .filter(action_targets::action_id.eq(action_id_param))
            .select((action_targets::device_id, action_targets::status, action_targets::response))
            .load::<(i64, String, Option<String>)>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;
        Ok(results)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(targets))
}

/// Update action TTL (use DB helper which enforces server default)
#[post("/actions/update_ttl/<action_id>/<ttl_seconds>")]
pub async fn update_action_ttl(
    pool: &State<DbPool>,
    action_id: i64,
    ttl_seconds: i64,
) -> Result<Status, Status> {
    let pool_inner = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        // Load server settings row to enforce default TTL limits
        use crate::schema::server_settings::dsl as ss_dsl;
        let settings_row: ServerSettingsRow = ss_dsl::server_settings.first(&mut conn).map_err(|_| Status::InternalServerError)?;
        db_update_action_ttl(&mut conn, action_id, ttl_seconds, &settings_row).map_err(|_| Status::InternalServerError)?;
        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Status::Ok)
}

/// Get remaining TTL (seconds) for an action
#[get("/actions/ttl/<action_id>")]
pub async fn get_action_ttl(
    pool: &State<DbPool>,
    action_id: i64,
) -> Result<Json<i64>, Status> {
    let pool_inner = pool.inner().clone();
    let remaining = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        db_fetch_action_ttl(&mut conn, action_id).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Json(remaining))
}

/// Cleanup completed action targets older than now
#[delete("/actions/pending_cleanup")]
pub async fn pending_cleanup(
    pool: &State<DbPool>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool_inner = pool.inner().clone();
    let username = user.username.clone();

    let res = rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        let cutoff = Utc::now().naive_utc();
        diesel::delete(
            action_targets::table
                .filter(action_targets::status.eq("completed"))
                .filter(action_targets::last_update.lt(cutoff)),
        )
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let _ = crate::routes::history::log_audit(&pool.inner(), &username, "action.pending_cleanup", None, None).await;

    Ok(Status::Ok)
}

/// Report result for an action on a specific device
#[post("/actions/report_result/<action_id_param>/<device_id_param>", data = "<body>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    action_id_param: i64,
    device_id_param: i64,
    body: String,
) -> Result<Status, Status> {
    let pool_inner = pool.inner().clone();
    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool_inner.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(
            action_targets::table
                .filter(action_targets::action_id.eq(action_id_param))
                .filter(action_targets::device_id.eq(device_id_param)),
        )
        .set((
            action_targets::response.eq(Some(body)),
            action_targets::status.eq("completed"),
            action_targets::last_update.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;
    Ok(Status::Ok)
}
