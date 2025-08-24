use diesel::prelude::Insertable;
use diesel::Queryable;
use diesel::Selectable;
use uuid::Uuid;
use crate::database::schema::users;

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
