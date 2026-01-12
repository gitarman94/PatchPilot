// src/auth.rs
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{Redirect, content::RawHtml};
use rocket::State;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use bcrypt::verify;
use std::fs::read_to_string;

use crate::db::{DbPool, log_audit};
use crate::schema::{users, roles, user_roles};

#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub role: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    User,
}

impl UserRole {
    /// String representation for storage/comparison.
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::User => "User",
        }
    }
}

impl AuthUser {
    pub fn has_role(&self, role: UserRole) -> bool {
        match role {
            UserRole::Admin => self.role == UserRole::Admin.as_str(),
            UserRole::User => true,
        }
    }

    pub fn audit(
        &self,
        conn: &mut SqliteConnection,
        action: &str,
        target: Option<&str>,
    ) {
        let _ = log_audit(conn, &self.username, action, target, None);
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Extract user_id cookie
        let user_id: i32 = match req
            .cookies()
            .get_private("user_id")
            .and_then(|c| c.value().parse().ok())
        {
            Some(id) => id,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        // Get DB pool state
        let pool = match req.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Acquire connection
        let mut conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Query user and role
        let result = users::table
            .filter(users::id.eq(user_id))
            .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
            .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
            .select((users::id, users::username, roles::name.nullable()))
            .first::<(i32, String, Option<String>)>(&mut conn);

        match result {
            Ok((id, username, role_opt)) => {
                let role = role_opt
                    .as_deref()
                    .map(UserRole::from_name)
                    .unwrap_or(UserRole::User);

                Outcome::Success(AuthUser { id, username, role })
            }
            Err(_) => Outcome::Error((Status::Unauthorized, ())),
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

    let result = users::table
        .filter(users::username.eq(&form.username))
        .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
        .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
        .select((users::id, users::username, users::password_hash, roles::name.nullable()))
        .first::<(i32, String, String, Option<String>)>(&mut conn);

    let (id, username, hash, role_opt) = match result {
        Ok(r) => r,
        Err(_) => return Redirect::to("/login"),
    };

    if !verify(&form.password, &hash).unwrap_or(false) {
        return Redirect::to("/login");
    }

    // Set private cookie (use CookieBuilder::build() instead of deprecated finish())
    let mut cookie = Cookie::new("user_id", id.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookies.add_private(cookie);

    let user = AuthUser {
        id,
        username,
        role: role_opt.unwrap_or_else(|| UserRole::User.as_str().to_string()),
    };

    user.audit(&mut conn, "login", None);

    Redirect::to("/dashboard")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let user_id = cookies
        .get_private("user_id")
        .and_then(|c| c.value().parse::<i32>().ok());

    // Use Cookie::build(...).build() (finish was deprecated)
    cookies.remove_private(Cookie::build("user_id").build());

    if let Some(uid) = user_id {
        if let Ok(mut conn) = pool.get() {
            if let Ok((username, role_opt)) = users::table
                .filter(users::id.eq(uid))
                .left_outer_join(user_roles::table.on(user_roles::user_id.eq(users::id)))
                .left_outer_join(roles::table.on(roles::id.eq(user_roles::role_id)))
                .select((users::username, roles::name.nullable()))
                .first::<(String, Option<String>)>(&mut conn)
            {
                let user = AuthUser {
                    id: uid,
                    username,
                    role: role_opt.unwrap_or_else(|| UserRole::User.as_str().to_string()),
                };
                user.audit(&mut conn, "logout", None);
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
