use rocket::{get, post, delete, State};
use rocket::serde::json::Json;
use rocket::form::Form;
use rocket::http::Status;
use diesel::prelude::*;
use chrono::{Utc, Duration};
use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Action, NewAction};
use crate::schema::{actions, action_targets};
use crate::routes::history::log_audit;
use uuid::Uuid;

#[derive(FromForm)]
pub struct SubmitActionForm {
    pub command: String,
    pub target_device_id: i32,
    pub ttl_seconds: Option<i64>,
}

#[post("/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Form<SubmitActionForm>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let user_name = user.username.clone();
    let form = form.into_inner();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        let ttl = form.ttl_seconds.unwrap_or(3600);
        let expires_at = Utc::now().naive_utc() + Duration::seconds(ttl);

        // Fixed: NewAction fields must match struct
        let new_action = NewAction {
            id: Uuid::new_v4().to_string(),
            action_type: form.command.clone(),   // Replaced `command` field
            parameters: None,                     // Optional parameters
            author: Some(user_name.clone()),      // author replaces created_by
            created_at: Utc::now().naive_utc(),
            expires_at,
            canceled: false,
        };

        diesel::insert_into(actions::table)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        // Fixed: device_id is Integer in schema
        diesel::insert_into(action_targets::table)
            .values((
                action_targets::action_id.eq(&new_action.id),
                action_targets::device_id.eq(&form.target_device_id),
                action_targets::status.eq("pending"),
                action_targets::last_update.eq(Utc::now().naive_utc()),
                action_targets::response.eq::<Option<String>>(None),
            ))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        // Audit logging
        log_audit(
            &mut conn,
            &user_name,
            "action.submit",
            Some(&new_action.id),
            Some(&format!("Target device: {}", form.target_device_id)),
        )
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Created)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[get("/actions")]
pub async fn list_actions(pool: &State<DbPool>, user: AuthUser) -> Result<Json<Vec<Action>>, Status> {
    let pool = pool.inner().clone();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Json<Vec<Action>>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let all_actions = actions::table
            .order(actions::created_at.desc())
            .load::<Action>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &user_name, "action.list", None, None).ok();

        Ok(Json(all_actions))
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[post("/actions/cancel/<action_id_param>")]
pub async fn cancel_action(
    pool: &State<DbPool>,
    action_id_param: &str,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_id = action_id_param.to_string();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(actions::table.filter(actions::id.eq(&action_id)))
            .set(actions::canceled.eq(true))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &user_name, "action.cancel", Some(&action_id), None)
            .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[get("/actions/targets/<action_id_param>")]
pub async fn list_action_targets(
    pool: &State<DbPool>,
    action_id_param: &str,
    user: AuthUser,
) -> Result<Json<Vec<(i32, String, Option<String>)>>, Status> {
    let pool = pool.inner().clone();
    let action_id = action_id_param.to_string();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Json<Vec<(i32, String, Option<String>)>>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let targets = action_targets::table
            .filter(action_targets::action_id.eq(&action_id))
            .select((action_targets::device_id, action_targets::status, action_targets::response))
            .load::<(i32, String, Option<String>)>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &user_name, "action.list_targets", Some(&action_id), None).ok();

        Ok(Json(targets))
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[post("/actions/update_ttl/<action_id_param>/<ttl_seconds>")]
pub async fn update_action_ttl(
    pool: &State<DbPool>,
    action_id_param: &str,
    ttl_seconds: i64,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_id = action_id_param.to_string();
    let user_name = user.username.clone();
    let new_expiry = Utc::now().naive_utc() + Duration::seconds(ttl_seconds);

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        diesel::update(actions::table.filter(actions::id.eq(&action_id)))
            .set(actions::expires_at.eq(new_expiry))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        log_audit(
            &mut conn,
            &user_name,
            "action.ttl_update",
            Some(&action_id),
            Some(&format!("New TTL: {} seconds", ttl_seconds)),
        )
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[delete("/actions/pending_cleanup")]
pub async fn pending_cleanup(pool: &State<DbPool>, user: AuthUser) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let cutoff = Utc::now().naive_utc();
        diesel::delete(
            action_targets::table
                .filter(action_targets::status.eq("completed")
                .and(action_targets::last_update.lt(cutoff)))
        )
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &user_name, "action.pending_cleanup", None, None).ok();

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}
