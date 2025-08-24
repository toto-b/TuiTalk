mod wsserver;
mod database;

use std::sync::Arc;
use redis::Connection;
use tokio::sync::Mutex;
use database::connection::establish_connection;
use database::queries::*;
use database::models::User;
use diesel::prelude::*;
use database::schema::users::dsl::*;
use ::uuid::Uuid;
use dotenvy::dotenv;

use crate::database::models::NewUser; 

type SharedRedis = Arc<Mutex<Connection>>;


#[tokio::main]
async fn main() -> redis::RedisResult<()> {
    dotenv().ok(); 
    let mut db_connection = establish_connection();
    println!("Connected to database");

    let _ = insert_user(&mut db_connection, NewUser { room_id: 10, uuid: Uuid::new_v4() });
    let result = users.select(User::as_select()).load::<User>(&mut db_connection);
    for user in result.unwrap() {
        println!("User in db {:?}", user);
    }


    let server_handle = tokio::spawn(async move {
        wsserver::start_ws_server().await.expect("Server failed");
    });

    tokio::select! {
        _ = server_handle => println!("Server stopped"),
    }

    Ok(())
}
