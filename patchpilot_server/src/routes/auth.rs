use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;
use diesel::prelude::*;
use diesel::SelectableHelper;
use std::fs::read_to_string;
use bcrypt::verify;

use crate::db::DbPool;
use crate::schema::users;

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

// AuthUser request guard
#[derive(Clone)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookies = req.cookies();
        let pool = req.guard::<&State<DbPool>>().await.succeeded();

        match (cookies.get_private("user_id"), pool) {
            (Some(cookie), Some(pool)) => {
                if let Ok(user_id) = cookie.value().parse::<i32>() {
                    let pool = pool.inner().clone();
                    let user_opt = rocket::tokio::task::spawn_blocking(move || {
                        let mut conn = pool.get().ok()?;
                        users::table
                            .filter(users::id.eq(user_id))
                            .first::<UserRow>(&mut conn)
                            .optional()
                            .ok()?
                    })
                    .await
                    .ok()
                    .flatten();

                    if let Some(user) = user_opt {
                        return Outcome::Success(AuthUser {
                            id: user.id,
                            username: user.username,
                        });
                    }
                }
                Outcome::Failure((Status::Unauthorized, ()))
            }
            _ => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

#[post("/login", data = "<form>")]
pub fn login(
    form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
    pool: &State<DbPool>,
) -> Redirect {
    use crate::schema::users::dsl::*;

    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    let user_opt = users
        .filter(username.eq(&form.username))
        .select(UserRow::as_select())
        .first::<UserRow>(&mut conn)
        .optional()
        .unwrap_or(None);

    if let Some(user) = user_opt {
        if verify(&form.password, &user.password_hash).unwrap_or(false) {
            cookies.add_private(Cookie::new("user_id", user.id.to_string()));

            let _ = crate::routes::history::log_audit(
                &mut conn,
                &user.username,
                "auth.login.success",
                None,
                Some("User login successful"),
            );

            return Redirect::to("/dashboard");
        }

        let _ = crate::routes::history::log_audit(
            &mut conn,
            &user.username,
            "auth.login.failure",
            None,
            Some("Invalid password provided"),
        );
    }

    Redirect::to("/login")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    if let Some(cookie) = cookies.get_private("user_id") {
        if let Ok(user_id) = cookie.value().parse::<i32>() {
            if let Ok(mut conn) = pool.get() {
                let _ = crate::routes::history::log_audit(
                    &mut conn,
                    &user_id.to_string(),
                    "auth.logout",
                    None,
                    Some("User logout"),
                );
            }
        }
    }

    // Fixed deprecated CookieBuilder::finish()
    cookies.remove_private(Cookie::new("user_id", ""));

    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".into()),
    )
}
