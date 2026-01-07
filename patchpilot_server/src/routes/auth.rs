use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use diesel::prelude::*;
use diesel::SelectableHelper;

use crate::db::DbPool;
use crate::schema::{users, audit};
use crate::models::AuditLog;

use bcrypt::verify;
use std::fs::read_to_string;
use chrono::Utc;

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Queryable, Selectable, Clone, Debug)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookies = request.cookies();
        let Some(cookie) = cookies.get_private("user_id") else {
            return Outcome::Failure((Status::Unauthorized, ()));
        };

        let Ok(user_id) = cookie.value().parse::<i32>() else {
            return Outcome::Failure((Status::Unauthorized, ()));
        };

        let pool = match request.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Failure((Status::InternalServerError, ())),
        };

        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return Outcome::Failure((Status::InternalServerError, ())),
        };

        use crate::schema::users::dsl::*;

        match users
            .filter(id.eq(user_id))
            .select((id, username))
            .first::<(i32, String)>(&mut conn)
        {
            Ok((id, username)) => Outcome::Success(AuthUser { id, username }),
            Err(_) => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

#[post("/login", data = "<form>")]
pub fn login(
    form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
    pool: &State<DbPool>,
) -> Redirect {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    use crate::schema::users::dsl::*;

    let user = match users
        .filter(username.eq(&form.username))
        .first::<UserRow>(&mut conn)
        .optional()
    {
        Ok(Some(u)) => u,
        _ => return Redirect::to("/login"),
    };

    if !verify(&form.password, &user.password_hash).unwrap_or(false) {
        return Redirect::to("/login");
    }

    let mut cookie = Cookie::new("user_id", user.id.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookies.add_private(cookie);

    // audit: login event
    let new_audit = (
        audit::actor.eq(user.username.clone()),
        audit::action_type.eq("login"),
        audit::target.eq::<Option<String>>(None),
        audit::details.eq::<Option<String>>(None),
        audit::created_at.eq(Utc::now().naive_utc()),
    );
    let _ = diesel::insert_into(audit::table)
        .values(new_audit)
        .execute(&mut conn);

    Redirect::to("/dashboard")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    // capture user id before removal
    let user_id_opt = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    cookies.remove_private(Cookie::named("user_id"));

    if let Some(user_id) = user_id_opt {
        if let Ok(mut conn) = pool.get() {
            use crate::schema::users::dsl::*;
            if let Ok(username) = users
                .filter(id.eq(user_id))
                .select(username)
                .first::<String>(&mut conn)
            {
                let new_audit = (
                    audit::actor.eq(username),
                    audit::action_type.eq("logout"),
                    audit::target.eq::<Option<String>>(None),
                    audit::details.eq::<Option<String>>(None),
                    audit::created_at.eq(Utc::now().naive_utc()),
                );
                let _ = diesel::insert_into(audit::table)
                    .values(new_audit)
                    .execute(&mut conn);
            }
        }
    }

    Redirect::to("/login")
}

#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".to_string()),
    )
}
