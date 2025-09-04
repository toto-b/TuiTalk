use crate::database::models::{Message, NewMessage, NewUser, User};
use crate::database::schema::messages::{self, dsl::room_id as msg_room_id, dsl::*};
use crate::database::schema::users::dsl::uuid;
use crate::database::schema::users::dsl::*;
use ::uuid::Uuid;
use diesel::associations::HasTable;
use diesel::prelude::*;

pub fn insert_user(conn: &mut PgConnection, user: NewUser) -> Result<usize, diesel::result::Error> {
    diesel::insert_into(users::table())
        .values(user)
        .execute(conn)
}

#[allow(dead_code)]
pub fn get_users(conn: &mut PgConnection) -> Result<Vec<User>, diesel::result::Error> {
    users.load::<User>(conn)
}

#[allow(dead_code)]
pub fn get_messages(conn: &mut PgConnection) -> QueryResult<Vec<Message>> {
    messages.load::<Message>(conn)
}

pub fn insert_message(
    conn: &mut PgConnection,
    msg: NewMessage,
) -> Result<usize, diesel::result::Error> {
    diesel::insert_into(messages::table)
        .values(msg)
        .execute(conn)
}

pub fn delete_user_by_uuid(
    conn: &mut PgConnection,
    user_uuid: Uuid,
) -> Result<usize, diesel::result::Error> {
    diesel::delete(users.filter(uuid.eq(user_uuid))).execute(conn)
}

pub fn get_history(
    conn: &mut PgConnection,
    requested_room_id: &i32,
    limit: &i64,
    fetch_before: &u64,
) -> Result<Vec<Message>, diesel::result::Error> {
    let mut result = messages
        .filter(
            msg_room_id
                .eq(*requested_room_id)
                .and(time.lt(*fetch_before as i64)),
        )
        .order_by(time.desc())
        .limit(*limit)
        .select(Message::as_select())
        .load::<Message>(conn)?;

    result.sort_by_key(|e| e.time);
    Ok(result)
}

pub fn get_room_id_by_uuid(conn: &mut PgConnection, user_uuid: Uuid) -> QueryResult<Vec<User>> {
    users
        .filter(uuid.eq(user_uuid))
        .limit(1)
        .select(User::as_select())
        .load::<User>(conn)
}
