use rocket::request::{FromRequest, Outcome, Request};
use rocket::http::Status;
use rocket::State;
use diesel::prelude::*;
use crate::db::DbPool;
use crate::schema::{users, roles, user_roles};

#[derive(Debug, Clone, PartialEq)]
pub enum UserRole {
    Admin,
    Manager,
    User,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i32,
    pub username: String,
    pub roles: Vec<UserRole>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie = req.cookies().get_private("user_id");
        let pool = match req.guard::<&State<DbPool>>().await {
            Outcome::Success(p) => p,
            _ => return Outcome::Failure((Status::InternalServerError, ())),
        };

        if let Some(cookie) = cookie {
            let user_id: i32 = cookie.value().parse().unwrap_or(0);
            let conn = pool.get().expect("Failed to get DB connection");

            // Fetch username
            let username_result = users::table
                .filter(users::id.eq(user_id))
                .select(users::username)
                .first::<String>(&conn)
                .optional()
                .expect("DB query failed");

            if let Some(username) = username_result {
                // Fetch roles
                let role_names = user_roles::table
                    .inner_join(roles::table.on(roles::id.eq(user_roles::role_id)))
                    .filter(user_roles::user_id.eq(user_id))
                    .select(roles::name)
                    .load::<String>(&conn)
                    .map_err(|_| Status::InternalServerError)?;

                let mut roles_vec = Vec::new();
                for role_name in role_names {
                    match role_name.as_str() {
                        "Admin" => roles_vec.push(UserRole::Admin),
                        "Manager" => roles_vec.push(UserRole::Manager),
                        _ => roles_vec.push(UserRole::User),
                    }
                }

                return Outcome::Success(AuthUser {
                    id: user_id,
                    username,
                    roles: roles_vec,
                });
            }
        }

        Outcome::Failure((Status::Unauthorized, ()))
    }
}

impl AuthUser {
    pub fn has_role(&self, role: &UserRole) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}
