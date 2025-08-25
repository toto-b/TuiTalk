use diesel::prelude::Insertable;
use diesel::Queryable;
use diesel::Selectable;
use uuid::Uuid;
use crate::database::schema::users;
use crate::database::schema::messages;

#[allow(unused)]
#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub room_id: i32,
    pub uuid: Uuid,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser {
    pub room_id: i32,
    pub uuid: Uuid,
}

#[allow(unused)]
#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Message {
    pub id: i32,
    pub time: i32, // i64 is purposely choose since diesel doesn't support u64
    pub username: String,
    pub message: String,
    pub room_id: i32,
    pub uuid: Uuid,
}

#[derive(Insertable, Debug)]
#[diesel(table_name = messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewMessage {
    pub time: i32,
    pub username: String,
    pub message: String,
    pub room_id: i32,
    pub uuid: Uuid,
}
