use diesel::prelude::*;
use diesel::associations::HasTable;
use crate::database::models::{NewMessage, NewUser, Message ,User};
use crate::database::schema::users::dsl::*;
use crate::database::schema::users::dsl::uuid;
use crate::database::schema::messages::{self, dsl::*};
use ::uuid::Uuid;   

pub fn insert_user(conn: &mut PgConnection, user: NewUser) -> Result<usize, diesel::result::Error> {
    diesel::insert_into(users::table())
        .values(user)
        .execute(conn)
}

pub fn get_users(conn: &mut PgConnection) -> Result<Vec<User>,diesel::result::Error> {
    users.load::<User>(conn)
}


pub fn get_messages(conn: &mut PgConnection) -> QueryResult<Vec<Message>>  {
    messages.load::<Message>(conn)
}

pub fn insert_message(conn: &mut PgConnection, msg: NewMessage) -> Result<usize, diesel::result::Error> {
    diesel::insert_into(messages::table)
        .values(msg)
        .execute(conn)
}

pub fn delete_user_by_uuid(
    conn: &mut PgConnection, 
    user_uuid: Uuid
) -> Result<usize, diesel::result::Error> {
    diesel::delete(users.filter(uuid.eq(user_uuid)))
        .execute(conn)
}
