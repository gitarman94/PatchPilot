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

/// Role enum used throughout the codebase (replace the previous `role: String`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    User,
}

impl UserRole {
    /// Parse a role name (case-sensitive matching of stored DB role names)
    pub fn from_name(name: &str) -> Self {
        match name {
            "Admin" => UserRole::Admin,
            _ => UserRole::User,
        }
    }

    /// Return the canonical role name string if needed
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "Admin",
            UserRole::User => "User",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub role: UserRole,
}

impl AuthUser {
    /// Check role membership. `User` is the least-privileged role and matches everyone.
    pub fn has_role(&self, role: UserRole) -> bool {
        match role {
            UserRole::Admin => self.role == UserRole::Admin,
            UserRole::User => true,
        }
    }

    /// Audit helper — include the user id in details so the `id` field is actually used.
    /// We intentionally ignore the result here (most callers use it as a best-effort log).
    pub fn audit(&self, conn: &mut SqliteConnection, action: &str, target: Option<&str>) {
        let details_string = format!("user_id: {}", self.id);
        let _ = log_audit(conn, &self.username, action, target, Some(&details_string));
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get user id from private cookie
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

        // Query user + role name (nullable)
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

    // Fetch id, username, password_hash and nullable role name
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

    let mut cookie = Cookie::new("user_id", id.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookies.add_private(cookie);

    let user = AuthUser {
        id,
        username,
        role: role_opt.as_deref().map(UserRole::from_name).unwrap_or(UserRole::User),
    };

    user.audit(&mut conn, "login", None);

    Redirect::to("/dashboard")
}

#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    // Read user id if present
    let user_id_opt = cookies.get_private("user_id").and_then(|c| c.value().parse::<i32>().ok());
    cookies.remove_private(Cookie::build("user_id").finish());

    if let Some(uid) = user_id_opt {
        if let Ok(mut conn) = pool.get() {
            // Query username and role name
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
                    role: role_opt.as_deref().map(UserRole::from_name).unwrap_or(UserRole::User),
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
