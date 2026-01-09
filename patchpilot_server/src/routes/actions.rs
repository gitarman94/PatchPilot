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

#[post("/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Form<SubmitActionForm>,
    user: AuthUser,
) -> Result<Status, Status> {
    // Separate clones: one for DB work, one to pass to audit logging after DB work completes.
    let pool_for_db = pool.inner().clone();
    let pool_for_audit = pool.inner().clone();
    let form = form.into_inner();
    let username = user.username.clone();
    let username_for_audit = username.clone();

    // Insert action + target + history within a blocking task (synchronous DB access).
    let action_id = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;
        let now = Utc::now().naive_utc();

        // Load server settings to determine default/cap TTL
        let settings_row: ServerSettingsRow =
            db_load_settings(&mut conn).map_err(|_| Status::InternalServerError)?;

        // Determine TTL to use: if provided, cap it to server default; otherwise use default.
        let ttl_seconds = match form.ttl_seconds {
            Some(v) => std::cmp::min(v, settings_row.default_action_ttl_seconds),
            None => settings_row.default_action_ttl_seconds,
        };

        let expires_at = now + Duration::seconds(ttl_seconds);

        // NewAction uses the project's fields (action_type, parameters, author, created_at, expires_at, canceled)
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
            .map_err(|_| Status::InternalServerError)?;

        // Retrieve last inserted action id (SQLite: query highest id)
        let action_id: i64 = actions::table
            .select(actions::id)
            .order(actions::id.desc())
            .first(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        // Insert the single target for this action (use the NewActionTarget helper or construct directly)
        let new_target = NewActionTarget::pending(action_id, form.target_device_id);

        diesel::insert_into(action_targets::table)
            .values(&new_target)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        // Insert history entry (using DB helper that accepts NewHistory<'_>)
        let history = crate::db::NewHistory {
            action_id,
            device_name: None,
            actor: Some(&username),
            action_type: "action.submit",
            details: None,
            created_at: now,
        };

        db_insert_history(&mut conn, &history).map_err(|_| Status::InternalServerError)?;

        Ok(action_id)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    // Audit log (best-effort, async)
    let _ = crate::routes::history::log_audit(
        &pool_for_audit,
        &username_for_audit,
        "action.submit",
        Some(&action_id.to_string()),
        Some(&format!("target_device_id={}", form.target_device_id)),
    )
    .await;

    Ok(Status::Created)
}

#[get("/actions")]
pub async fn list_actions(
    pool: &State<DbPool>,
    _user: AuthUser,
) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();

    let actions = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<Action>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        actions::table
            .order(actions::created_at.desc())
            .load::<Action>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(actions))
}

#[post("/actions/cancel/<action_id>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id: i64,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool_for_db = pool.inner().clone();
    let pool_for_audit = pool.inner().clone();
    let username = user.username.clone();
    let username_for_audit = username.clone();

    let _ = rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;
        let now = Utc::now().naive_utc();

        diesel::update(actions::table.filter(actions::id.eq(action_id)))
            .set(actions::expires_at.eq(now))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        let history = crate::db::NewHistory {
            action_id,
            device_name: None,
            actor: Some(&username),
            action_type: "action.cancel",
            details: None,
            created_at: now,
        };

        db_insert_history(&mut conn, &history).map_err(|_| Status::InternalServerError)?;

        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    let _ = crate::routes::history::log_audit(
        &pool_for_audit,
        &username_for_audit,
        "action.cancel",
        Some(&action_id.to_string()),
        None,
    )
    .await;

    Ok(Status::Ok)
}

#[get("/actions/targets/<action_id>")]
pub async fn list_action_targets(
    pool: &State<DbPool>,
    action_id: i64,
) -> Result<Json<Vec<(i64, String, Option<String>)>>, Status> {
    let pool = pool.inner().clone();

    let targets = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<(i64, String, Option<String>)>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        action_targets::table
            .filter(action_targets::action_id.eq(action_id))
            .select((
                action_targets::device_id,
                action_targets::status,
                action_targets::response,
            ))
            .load::<(i64, String, Option<String>)>(&mut conn)
            .map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(targets))
}

#[post("/actions/update_ttl/<action_id>/<ttl_seconds>")]
pub async fn update_action_ttl(
    pool: &State<DbPool>,
    action_id: i64,
    ttl_seconds: i64,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        use crate::schema::server_settings::dsl as ss;
        let settings: ServerSettingsRow =
            ss::server_settings.first(&mut conn).map_err(|_| Status::InternalServerError)?;

        db_update_action_ttl(&mut conn, action_id, ttl_seconds, &settings)
            .map_err(|_| Status::InternalServerError)?;

        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Status::Ok)
}

#[get("/actions/ttl/<action_id>")]
pub async fn get_action_ttl(
    pool: &State<DbPool>,
    action_id: i64,
) -> Result<Json<i64>, Status> {
    let pool = pool.inner().clone();

    let ttl = rocket::tokio::task::spawn_blocking(move || -> Result<i64, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        db_fetch_action_ttl(&mut conn, action_id).map_err(|_| Status::InternalServerError)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(ttl))
}

#[delete("/actions/pending_cleanup")]
pub async fn pending_cleanup(
    pool: &State<DbPool>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool_for_db = pool.inner().clone();
    let pool_for_audit = pool.inner().clone();
    let username = user.username.clone();
    let username_for_audit = username.clone();

    let _ = rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool_for_db.get().map_err(|_| Status::InternalServerError)?;
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

    let _ = crate::routes::history::log_audit(
        &pool_for_audit,
        &username_for_audit,
        "action.pending_cleanup",
        None,
        None,
    )
    .await;

    Ok(Status::Ok)
}

#[post("/actions/report_result/<action_id>/<device_id>", data = "<body>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    action_id: i64,
    device_id: i64,
    body: String,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<(), Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::update(
            action_targets::table
                .filter(action_targets::action_id.eq(action_id))
                .filter(action_targets::device_id.eq(device_id)),
        )
        .set((
            action_targets::response.eq(Some(body)),
            action_targets::status.eq("Completed".to_string()),
            action_targets::last_update.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

        Ok(())
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Status::Ok)
}
