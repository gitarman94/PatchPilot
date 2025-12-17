use rocket::serde::json::Json;
use rocket::{get, post, State, http::Status};
use diesel::prelude::*;
use crate::models::{Action, NewAction, ActionTarget};
use crate::DbPool;
use crate::schema::actions::dsl::*;
use crate::schema::action_targets::dsl::*;

// Submit new action
#[post("/api/actions", data = "<action>")]
pub async fn submit_action(
    pool: &State<DbPool>,
    action: Json<NewAction>,
) -> Result<Json<Action>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let new_action = diesel::insert_into(actions)
        .values(&*action)
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    // Retrieve the inserted action
    let inserted_action = actions
        .order(id.desc())
        .first::<Action>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(inserted_action))
}

// List all actions
#[get("/api/actions")]
pub async fn list_actions(pool: &State<DbPool>) -> Result<Json<Vec<Action>>, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;
    let all_actions = actions
        .load::<Action>(&mut conn)
        .map_err(|_| Status::InternalServerError)?;
    Ok(Json(all_actions))
}

// Cancel an action
#[post("/api/actions/<action_id>/cancel")]
pub async fn cancel_action(pool: &State<DbPool>, action_id: i32) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(actions.filter(id.eq(action_id)))
        .set(status.eq("canceled"))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}

// Report action result
#[post("/api/actions/result", data = "<result>")]
pub async fn report_action_result(
    pool: &State<DbPool>,
    result: Json<ActionTarget>,
) -> Result<Status, Status> {
    let mut conn = pool.get().map_err(|_| Status::InternalServerError)?;

    diesel::update(action_targets.filter(action_id.eq(result.action_id).and(target.eq(&result.target))))
        .set(status.eq(&result.status))
        .execute(&mut conn)
        .map_err(|_| Status::InternalServerError)?;

    Ok(Status::Ok)
}
