// src/auth.rs
use rocket::form::Form;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use rocket::response::{Redirect, content::RawHtml};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::State;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use bcrypt::verify;
use chrono::Utc;
use std::fs::read_to_string;

use crate::db::DbPool;
use crate::db::log_audit;
use crate::schema::{audit, users, roles, user_roles};
use crate::models::AuditLog;

/// Simple login form
#[derive(FromForm)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// Authenticated user type used as a request guard
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub role: String,
}

/// Simple role enumeration used by handlers
#[derive(Debug, PartialEq, Eq)]
pub enum UserRole {
    Admin,
    User,
}

impl AuthUser {
    /// Convenience: write an audit record using the project's db helper.
    /// Returns the Diesel QueryResult to allow caller to handle errors.
    pub fn audit(&self, conn: &mut SqliteConnection, action: &str, target: Option<&str>) -> QueryResult<()> {
        // Use the central log_audit helper defined in db.rs
        log_audit(conn, &self.username, action, target, None)
    }

    /// Check role membership; Admin only check implemented here.
    pub fn has_role(&self, role: UserRole) -> bool {
        match role {
            UserRole::Admin => self.role == "Admin",
            UserRole::User => true,
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Check for private cookie "user_id"
        let cookie_opt = request.cookies().get_private("user_id");

        let user_id: i32 = match cookie_opt.and_then(|c| c.value().parse::<i32>().ok()) {
            Some(id) => id,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        // Get DB pool managed state
        let pool_guard = request.guard::<&State<DbPool>>().await;
        let pool_state = match pool_guard {
            Outcome::Success(p) => p.inner().clone(),
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Get a DB connection
        let mut conn = match pool_state.get() {
            Ok(c) => c,
            Err(_) => return Outcome::Error((Status::InternalServerError, ())),
        };

        // Query user and associated role (nullable)
        match users::table()
            .filter(users::id.eq(user_id))
            .left_outer_join(user_roles::table().on(user_roles::user_id.eq(users::id)))
            .left_outer_join(roles::table().on(roles::id.eq(user_roles::role_id)))
            .select((users::id, users::username, roles::name.nullable()))
            .first::<(i32, String, Option<String>)>(&mut conn)
        {
            Ok((uid, uname, urole)) => {
                let role = urole.unwrap_or_else(|| "User".to_string());
                Outcome::Success(AuthUser { id: uid, username: uname, role })
            }
            Err(_) => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}

/// POST /auth/login - process a login form
#[post("/login", data = "<form>")]
pub fn login(form: Form<LoginForm>, cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    let mut conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return Redirect::to("/login"),
    };

    // Fetch user row and optional role
    let row_opt = users::table()
        .filter(users::username.eq(&form.username))
        .left_outer_join(user_roles::table().on(user_roles::user_id.eq(users::id)))
        .left_outer_join(roles::table().on(roles::id.eq(user_roles::role_id)))
        .select((users::id, users::username, users::password_hash, roles::name.nullable()))
        .first::<(i32, String, String, Option<String>)>(&mut conn)
        .optional();

    let row = match row_opt {
        Ok(Some(r)) => r,
        _ => return Redirect::to("/login"),
    };

    // Verify password hash
    if !verify(&form.password, &row.2).unwrap_or(false) {
        return Redirect::to("/login");
    }

    // Create cookie and set it
    let mut cookie = Cookie::new("user_id", row.0.to_string());
    cookie.set_same_site(SameSite::Lax);
    cookies.add_private(cookie);

    let auth_user = AuthUser {
        id: row.0,
        username: row.1.clone(),
        role: row.3.unwrap_or_else(|| "User".into()),
    };

    // Audit the login
    let _ = auth_user.audit(&mut conn, "login", None);

    Redirect::to("/dashboard")
}

/// GET /auth/logout - clear cookie and log
#[get("/logout")]
pub fn logout(cookies: &CookieJar<'_>, pool: &State<DbPool>) -> Redirect {
    // Grab the user id from cookie (if present) to log the logout
    let user_id_opt = cookies.get_private("user_id").and_then(|c| c.value().parse::<i32>().ok());

    // Remove cookie
    cookies.remove_private(Cookie::build("user_id").finish());

    if let Some(uid) = user_id_opt {
        if let Ok(mut conn) = pool.get() {
            // try to load username/role for auditing
            if let Ok((uname, urole)) = users::table()
                .filter(users::id.eq(uid))
                .left_outer_join(user_roles::table().on(user_roles::user_id.eq(users::id)))
                .left_outer_join(roles::table().on(roles::id.eq(user_roles::role_id)))
                .select((users::username, roles::name.nullable()))
                .first::<(String, Option<String>)>(&mut conn)
            {
                let auth_user = AuthUser {
                    id: uid,
                    username: uname,
                    role: urole.unwrap_or_else(|| "User".into()),
                };
                let _ = auth_user.audit(&mut conn, "logout", None);
            }
        }
    }

    Redirect::to("/login")
}

/// GET /auth/login - return login page HTML
#[get("/login")]
pub fn login_page() -> RawHtml<String> {
    RawHtml(
        read_to_string("templates/login.html")
            .unwrap_or_else(|_| "<h1>Login page missing</h1>".to_string()),
    )
}

/// Example token validation helper
pub async fn validate_token(token: &str) -> Result<AuthUser, ()> {
    // Example logic — replace with real token validation as needed.
    if token == "testtoken" {
        Ok(AuthUser {
            id: 1,
            username: "admin".into(),
            role: "Admin".into(),
        })
    } else {
        Err(())
    }
}
