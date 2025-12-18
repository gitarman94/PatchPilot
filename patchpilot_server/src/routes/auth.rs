use rocket::form::Form;
use rocket::http::CookieJar;
use rocket::response::Redirect;
use rocket::State;
use diesel::prelude::*;
use crate::db::DbPool;
use crate::auth::{AuthUser, UserRole};
use std::fs::read_to_string;
use crate::schema::users;

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[post("/login", data = "<form>")]
pub fn login(form: Form<LoginForm>, cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    use crate::schema::users::dsl::*;

    let mut conn = pool.get().expect("Failed to get DB connection");

    // Query user by username
    #[derive(Queryable)]
    struct UserRow {
        id: i32,
        username: String,
        password_hash: String,
    }

    let user_opt = users
        .filter(username.eq(&form.username))
        .first::<UserRow>(&mut conn)
        .optional()
        .expect("DB query failed");

    if let Some(user) = user_opt {
        if bcrypt::verify(&form.password, &user.password_hash).unwrap_or(false) {
            cookies.add_private(rocket::http::Cookie::new("user_id", user.id.to_string()));
            return Redirect::to("/dashboard"); // replace with actual route if needed
        }
    }

    Redirect::to("/login")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove_private(rocket::http::Cookie::build("user_id").finish());
    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> rocket::response::content::RawHtml<String> {
    rocket::response::content::RawHtml(
        read_to_string("templates/login.html").unwrap_or_else(|_| "<h1>Login page missing</h1>".into())
    )
}
