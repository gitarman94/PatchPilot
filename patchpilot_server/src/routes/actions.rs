use rocket::{get, post, delete, State};
use rocket::serde::json::Json;
use rocket::form::Form;
use rocket::http::Status;

use diesel::prelude::*;

use chrono::{Utc, Duration};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::db::DbPool;
use crate::models::{Action, NewAction};
use crate::schema::{actions, action_targets};
use crate::routes::history::log_audit;


#[derive(FromForm)]
pub struct SubmitActionForm {
    pub command: String,
    pub target_device_id: String,
    pub ttl_seconds: Option<i64>,
}

#[post("/actions/submit", data = "<form>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    form: Form<SubmitActionForm>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let form = form.into_inner();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        let ttl = form.ttl_seconds.unwrap_or(3600);
        let expires_at = Utc::now().naive_utc() + Duration::seconds(ttl);

        let new_action = NewAction {
            id: Uuid::new_v4().to_string(),
            action_type: form.command,
            parameters: None,
            author: Some(user_name.clone()),
            created_at: Utc::now().naive_utc(),
            expires_at,
            canceled: false,
        };

        diesel::insert_into(actions::table)
            .values(&new_action)
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        diesel::insert_into(action_targets::table)
            .values((
                action_targets::action_id.eq(&new_action.id),
                action_targets::device_id.eq(&form.target_device_id),
                action_targets::status.eq("pending".to_string()),
                action_targets::last_update.eq(Utc::now().naive_utc()),
                action_targets::response.eq::<Option<String>>(None),
            ))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

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
pub async fn list_actions(
    pool: &State<DbPool>,
    user: AuthUser,
) -> Result<Json<Vec<Action>>, Status> {
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
        let new_expiry = Utc::now().naive_utc() + Duration::seconds(ttl_seconds);
        diesel::update(actions::table.filter(actions::id.eq(&action_id)))
            .set(actions::expires_at.eq(new_expiry))
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
    _user: AuthUser,
) -> Result<Json<Vec<(String, String, Option<String>)>>, Status> {
    let pool = pool.inner().clone();
    let action_id_val = action_id_param.to_string();

    let targets = rocket::tokio::task::spawn_blocking(move || -> Result<Vec<(String, String, Option<String>)>, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        let results = action_targets::table
            .filter(action_targets::action_id.eq(&action_id_val))
            .select((action_targets::device_id, action_targets::status, action_targets::response))
            .load::<(String, String, Option<String>)>(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        Ok(results)
    })
    .await
    .map_err(|_| Status::InternalServerError)??;

    Ok(Json(targets))
}

#[post("/actions/update_ttl/<action_id>/<ttl_seconds>")]
pub async fn update_action_ttl(
    pool: &State<DbPool>,
    _user: AuthUser,
    action_id: &str,
    ttl_seconds: i64,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_id_val = action_id.to_string();

    rocket::tokio::task::spawn_blocking(move || -> Result<_, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::update(actions::table.filter(actions::id.eq(action_id_val)))
            .set(actions::ttl.eq(ttl_seconds))
            .execute(&mut conn)
            .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[delete("/actions/pending_cleanup")]
pub async fn pending_cleanup(
    pool: &State<DbPool>,
    user: AuthUser,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let user_name = user.username.clone();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
        let cutoff = Utc::now().naive_utc();

        diesel::delete(
            action_targets::table
                .filter(action_targets::status.eq("completed"))
                .filter(action_targets::last_update.lt(cutoff)),
        )
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

        log_audit(&mut conn, &user_name, "action.pending_cleanup", None, None).ok();

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}

#[post("/actions/report_result/<action_id_param>/<device_id_param>", data = "<body>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    action_id_param: &str,
    device_id_param: &str,
    body: String,
) -> Result<Status, Status> {
    let pool = pool.inner().clone();
    let action_id_val = action_id_param.to_string();
    let device_id_val = device_id_param.to_string();

    rocket::tokio::task::spawn_blocking(move || -> Result<Status, Status> {
        let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

        diesel::update(
            action_targets::table
                .filter(action_targets::action_id.eq(&action_id_val))
                .filter(action_targets::device_id.eq(&device_id_val)),
        )
        .set((
            action_targets::response.eq(Some(body)),
            action_targets::status.eq("completed".to_string()),
            action_targets::last_update.eq(Utc::now().naive_utc()),
        ))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

        Ok(Status::Ok)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
}
