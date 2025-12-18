use rocket::{get, post, delete, State};
use rocket::form::Form;
use rocket::response::Redirect;
use diesel::prelude::*;
use crate::db::{DbPool, log_audit};
use crate::auth::{AuthUser, UserRole};
use crate::schema::{users, groups, user_groups};
use rocket_dyn_templates::Template;
use std::collections::HashMap;

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
pub fn list_users_groups(user: AuthUser, pool: &State<DbPool>) -> Template {
    if !user.has_role(&UserRole::Admin) {
        return Template::render("unauthorized", &());
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("error", &"DB connection failed"),
    };

    // Load groups
    let all_groups = groups::table
        .load::<(i32, String, Option<String>)>(&conn)
        .unwrap_or_default();

    // Load users grouped by group
    let mut group_users: HashMap<i32, Vec<(i32, String)>> = HashMap::new();
    let joined = users::table
        .inner_join(user_groups::table.on(users::id.eq(user_groups::user_id)))
        .select((user_groups::group_id, users::id, users::username))
        .load::<(i32, i32, String)>(&conn)
        .unwrap_or_default();

    for (group_id, user_id, username) in joined {
        group_users.entry(group_id).or_default().push((user_id, username));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users
    });

    Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(user: AuthUser, pool: &State<DbPool>, form: Form<GroupForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    if diesel::insert_into(groups::table)
        .values((groups::name.eq(&form.name), groups::description.eq(&form.description)))
        .execute(&mut conn)
        .is_err() 
    {
        return Redirect::to("/error");
    }

    log_audit(&mut conn, &user.username, "add_group", Some(&form.name), form.description.as_deref());

    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(user: AuthUser, pool: &State<DbPool>, form: Form<UserForm>) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    let pass_hash = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return Redirect::to("/error"),
    };

    let user_id = match diesel::insert_into(users::table)
        .values((users::username.eq(&form.username), users::password_hash.eq(pass_hash)))
        .returning(users::id)
        .get_result::<i32>(&mut conn)
    {
        Ok(id) => id,
        Err(_) => return Redirect::to("/error"),
    };

    if let Some(group_id) = form.group_id {
        let _ = diesel::insert_into(user_groups::table)
            .values((user_groups::user_id.eq(user_id), user_groups::group_id.eq(group_id)))
            .execute(&mut conn);
    }

    let details = form.group_id.map(|id| format!("group_id: {}", id));
    log_audit(&mut conn, &user.username, "add_user", Some(&form.username), details.as_deref());

    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id>")]
pub fn delete_group(user: AuthUser, pool: &State<DbPool>, group_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    let group_name: String = groups::table
        .filter(groups::id.eq(group_id))
        .select(groups::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".to_string());

    let _ = diesel::delete(user_groups::table.filter(user_groups::group_id.eq(group_id)))
        .execute(&mut conn);
    let _ = diesel::delete(groups::table.filter(groups::id.eq(group_id)))
        .execute(&mut conn);

    log_audit(&mut conn, &user.username, "delete_group", Some(&group_name), None);

    Redirect::to("/users-groups")
}

#[delete("/users/<user_id>")]
pub fn delete_user(user: AuthUser, pool: &State<DbPool>, user_id: i32) -> Redirect {
    if !user.has_role(&UserRole::Admin) { return Redirect::to("/unauthorized"); }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/error"),
    };

    let username_val: String = users::table
        .filter(users::id.eq(user_id))
        .select(users::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".to_string());

    let _ = diesel::delete(user_groups::table.filter(user_groups::user_id.eq(user_id)))
        .execute(&mut conn);
    let _ = diesel::delete(users::table.filter(users::id.eq(user_id)))
        .execute(&mut conn);

    log_audit(&mut conn, &user.username, "delete_user", Some(&username_val), None);

    Redirect::to("/users-groups")
}
