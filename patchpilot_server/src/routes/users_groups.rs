use rocket::{get, post, routes, State};
use rocket::form::Form;
use rocket::FromForm;
use rocket::response::Redirect;
use diesel::prelude::*;

use crate::db::{DbPool, log_audit as db_log_audit};
use crate::auth::{AuthUser, RoleName};
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

#[get("/")]
pub fn list_users_groups(user: AuthUser, pool: &State<DbPool>) -> Template {
    if !user.has_role(RoleName::Admin) {
        return Template::render("unauthorized", &());
    }

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("error", &()),
    };

    let all_groups = groups::table
        .load::<(i32, String, Option<String>)>(&mut conn)
        .unwrap_or_default();

    let joined = users::table
        .inner_join(user_groups::table.on(users::id.eq(user_groups::user_id)))
        .select((user_groups::group_id, users::id, users::username))
        .load::<(i32, i32, String)>(&mut conn)
        .unwrap_or_default();

    let mut group_users: HashMap<i32, Vec<(i32, String)>> = HashMap::new();
    for (gid, uid, uname) in joined {
        group_users.entry(gid).or_default().push((uid, uname));
    }

    let context = serde_json::json!({
        "groups": all_groups,
        "group_users": group_users,
    });

    let _ = db_log_audit(&mut conn, &user.username, "users_groups.view", None, None);

    Template::render("users_groups", &context)
}

#[post("/groups/add", data = "<form>")]
pub fn add_group(user: AuthUser, pool: &State<DbPool>, form: Form<GroupForm>) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };
    let ff = form.into_inner();
    let _ = diesel::insert_into(groups::table)
        .values((groups::name.eq(&ff.name), groups::description.eq(&ff.description)))
        .execute(&mut conn);
    let _ = db_log_audit(&mut conn, &user.username, "group.create", Some(&ff.name), ff.description.as_deref());
    Redirect::to("/users-groups")
}

#[post("/users/add", data = "<form>")]
pub fn add_user(user: AuthUser, pool: &State<DbPool>, form: Form<UserForm>) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };
    let form = form.into_inner();
    let hashed = match bcrypt::hash(&form.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return Redirect::to("/users-groups"),
    };
    let new_user = (
        users::username.eq(&form.username),
        users::password_hash.eq(hashed),
    );
    let _ = diesel::insert_into(users::table).values(&new_user).execute(&mut conn);
    let new_id: i32 = users::table
        .order(users::id.desc())
        .select(users::id)
        .first(&mut conn)
        .unwrap_or(-1);
    if let Some(gid) = form.group_id {
        let _ = diesel::insert_into(user_groups::table)
            .values((user_groups::user_id.eq(new_id), user_groups::group_id.eq(gid)))
            .execute(&mut conn);
    }
    let details = form.group_id.map(|gid| format!("assigned_group: {}", gid));
    let _ = db_log_audit(&mut conn, &user.username, "user.create", Some(&form.username), details.as_deref());
    Redirect::to("/users-groups")
}

#[delete("/groups/<group_id_val>")]
pub fn delete_group(user: AuthUser, pool: &State<DbPool>, group_id_val: i32) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };
    let group_name: String = groups::table
        .filter(groups::id.eq(group_id_val))
        .select(groups::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());
    let _ = diesel::delete(user_groups::table.filter(user_groups::group_id.eq(group_id_val)))
        .execute(&mut conn);
    let _ = diesel::delete(groups::table.filter(groups::id.eq(group_id_val))).execute(&mut conn);
    let _ = db_log_audit(&mut conn, &user.username, "group.delete", Some(&group_name), None);
    Redirect::to("/users-groups")
}

#[delete("/users/<user_id_val>")]
pub fn delete_user(user: AuthUser, pool: &State<DbPool>, user_id_val: i32) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/users-groups"),
    };
    let username_val: String = users::table
        .filter(users::id.eq(user_id_val))
        .select(users::username)
        .first(&mut conn)
        .unwrap_or_else(|_| "unknown".into());
    let _ = diesel::delete(user_groups::table.filter(user_groups::user_id.eq(user_id_val)))
        .execute(&mut conn);
    let _ = diesel::delete(users::table.filter(users::id.eq(user_id_val))).execute(&mut conn);
    let _ = db_log_audit(&mut conn, &user.username, "user.delete", Some(&username_val), None);
    Redirect::to("/users-groups")
}

pub fn api_users_groups_routes() -> Vec<rocket::Route> {
    routes![
        list_users_groups,
        add_group,
        add_user,
        delete_group,
        delete_user
    ]
    .into_iter()
    .collect()
}
