use rocket::{get, post, delete, routes, State};
use rocket::form::Form;
use rocket::FromForm;
use rocket::response::Redirect;
use rocket_dyn_templates::Template;
use diesel::prelude::*;


use crate::db::{DbPool, log_audit, insert_history, insert_audit};

use crate::auth::{AuthUser, RoleName};
use crate::models::{Role as RoleModel};
use crate::schema::{roles, user_roles};

#[derive(FromForm)]
pub struct RoleForm {
    pub name: String,
}

#[get("/")]
pub fn list_roles(user: AuthUser, pool: &State<DbPool>) -> Template {
    if !user.has_role(RoleName::Admin) {
        return Template::render("unauthorized", &());
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Template::render("error", &()),
    };
    let all_roles = roles::table.load::<RoleModel>(&mut conn).unwrap_or_default();
    Template::render("roles", &all_roles)
}

#[post("/add", data = "<form>")]
pub fn add_role(user: AuthUser, pool: &State<DbPool>, form: Form<RoleForm>) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/roles"),
    };
    let ff = form.into_inner();
    let _ = diesel::insert_into(roles::table).values(roles::name.eq(&ff.name)).execute(&mut conn);
    let _ = log_audit(&mut conn, &user.username, "add_role", Some(&ff.name), None);
    Redirect::to("/roles")
}

#[delete("/<role_id>")]
pub fn delete_role(user: AuthUser, pool: &State<DbPool>, role_id: i32) -> Redirect {
    if !user.has_role(RoleName::Admin) {
        return Redirect::to("/unauthorized");
    }
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/roles"),
    };
    let role_name: String = roles::table
        .filter(roles::id.eq(role_id))
        .select(roles::name)
        .first(&mut conn)
        .unwrap_or_else(|_| "".into());
    let _ = diesel::delete(user_roles::table.filter(user_roles::role_id.eq(role_id))).execute(&mut conn);
    let _ = diesel::delete(roles::table.filter(roles::id.eq(role_id))).execute(&mut conn);
    let _ = log_audit(&mut conn, &user.username, "delete_role", Some(&role_name), None);
    Redirect::to("/roles")
}

pub fn api_roles_routes() -> Vec<rocket::Route> {
    routes![list_roles, add_role, delete_role].into_iter().collect()
}
