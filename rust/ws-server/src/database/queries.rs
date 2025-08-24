use diesel::prelude::*;
use diesel::associations::HasTable;
use crate::database::models::{NewUser, User};
use crate::database::schema::users::dsl::*; // brings `users` table into scope

pub fn insert_user(conn: &mut PgConnection, user: NewUser) -> Result<usize, diesel::result::Error> {
    diesel::insert_into(users::table())
        .values(user)
        .execute(conn)
}

pub fn get_users(conn: &mut PgConnection) -> Result<Vec<User>,diesel::result::Error> {
    use crate::database::schema::users::dsl::*;
    users.load::<User>(conn)
}
