use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use crate::db::{DbPool, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::schema::{users, groups, user_groups};

#[derive(FromForm)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub group_id: Option<i32>,
}

#[derive(FromForm)]
pub struct GroupForm {
    pub name: String,
    pub description: Option<String>,
}

#[get("/users-groups")]
pub fn list_users_groups(user: AuthUser, pool: &State<DbPool>) -> rocket_dyn_templates::Template {
    if !user.has_role(&UserRole::Admin) {
        return rocket_dyn_templates::Template::render("unauthorized", &());
    }

    let mut conn = pool.get().expect("Failed to get DB connection");

    let all_groups = groups::table
        .load::<(i32, String, Option<String>)>(&mut conn)
        .unwrap_or_default();

    let mut group_users: std::collections::HashMap<i32, Vec<(i32, String)>> = std::collections::HashMap::new();
    let joined = users::table
        .inner_join(user_groups::table.on(users::id.eq(user_groups::user_id)))
        .select((user_groups::group_id, users::id, users::username))
        .load::<(i32, i32, String)>(&mut conn)
        .unwrap_or_default();

    for (group_id, user_id, username) in joined {
        group_users.entry(group_id).or_default().push((user_id, username));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users
    });

    rocket_dyn_templates::Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(user: AuthUser, pool: &State<DbPool>, form: Form<GroupForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().expect("Failed to get DB connection");
    diesel::insert_into(groups::table)
        .values((groups::name.eq(&form.name), groups::description.eq(&form.description)))
        .execute(&mut conn)
        .unwrap();

    log_audit(&mut conn, &user.username, "add_group", Some(&form.name), form.description.as_deref())
        .unwrap();

    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(user: AuthUser, pool: &State<DbPool>, form: Form<UserForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().expect("Failed to get DB connection");
    let pass_hash = bcrypt::hash(&form.password, bcrypt::DEFAULT_COST).unwrap();

    // Insert the new user and return its ID
    let user_id: i32 = diesel::insert_into(users::table)
        .values((
            users::username.eq(&form.username),
            users::password_hash.eq(pass_hash),
        ))
        .returning(users::id)
        .get_result(&mut conn)
        .expect("Failed to insert user");

    // Add user to group if specified
    if let Some(group_id_val) = form.group_id {
        diesel::insert_into(user_groups::table)
            .values((
                user_groups::user_id.eq(user_id),
                user_groups::group_id.eq(group_id_val),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    let details = form.group_id.map(|id| format!("group_id: {}", id));
    let details_ref = details.as_deref();

    log_audit(&mut conn, &user.username, "add_user", Some(&form.username), details_ref)
        .unwrap();

    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id>")]
pub fn delete_group(user: AuthUser, pool: &State<DbPool>, group_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().expect("Failed to get DB connection");
    let group_name: String = groups::table
        .filter(groups::id.eq(group_id))
        .select(groups::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".to_string());

    diesel::delete(user_groups::table.filter(user_groups::group_id.eq(group_id)))
        .execute(&mut conn)
        .unwrap();
    diesel::delete(groups::table.filter(groups::id.eq(group_id)))
        .execute(&mut conn)
        .unwrap();

    log_audit(&mut conn, &user.username, "delete_group", Some(&group_name), None)
        .unwrap();

    Redirect::to("/users-groups")
}

#[delete("/users/<user_id>")]
pub fn delete_user(user: AuthUser, pool: &State<DbPool>, user_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = pool.get().expect("Failed to get DB connection");
    let username_val: String = users::table
        .filter(users::id.eq(user_id))
        .select(users::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".to_string());

    diesel::delete(user_groups::table.filter(user_groups::user_id.eq(user_id)))
        .execute(&mut conn)
        .unwrap();
    diesel::delete(users::table.filter(users::id.eq(user_id)))
        .execute(&mut conn)
        .unwrap();

    log_audit(&mut conn, &user.username, "delete_user", Some(&username_val), None)
        .unwrap();

    Redirect::to("/users-groups")
}
