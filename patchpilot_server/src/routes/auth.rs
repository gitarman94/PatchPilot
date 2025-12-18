use rocket::form::Form;
use rocket::http::CookieJar;
use rocket::response::Redirect;
use rocket::State;
use diesel::prelude::*;
use crate::db::DbPool;
use crate::auth::{AuthUser, UserRole};

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[post("/login", data = "<form>")]
pub fn login(form: Form<LoginForm>, cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    use crate::schema::users::dsl::*;

    let conn = pool.get().expect("Failed to get DB connection");

    // Query user by username
    let user_opt = users
        .filter(username.eq(&form.username))
        .first::<(i32, String, String)>(&conn)
        .optional()
        .expect("DB query failed");

    if let Some((user_id, _username, password_hash)) = user_opt {
        if bcrypt::verify(&form.password, &password_hash).unwrap_or(false) {
            // Success: set cookie
            cookies.add_private(rocket::http::Cookie::new("user_id", user_id.to_string()));
            return Redirect::to(uri!(crate::routes::dashboard));
        }
    }

    // Failure: redirect back to login
    Redirect::to(uri!(login_page))
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove_private(rocket::http::Cookie::named("user_id"));
    Redirect::to(uri!(login_page))
}

#[get("/login")]
pub fn login_page() -> rocket::response::content::RawHtml<String> {
    rocket::response::content::RawHtml(
        rocket::fs::read_to_string("templates/login.html").unwrap()
    )
}
