use diesel::prelude::*;

pub fn establish_connection() -> PgConnection {
    let db_host = std::env::var("POSTGRES_HOST").unwrap_or("localhost".to_string());
    let db_port = std::env::var("POSTGRES_PORT").unwrap_or("5432".to_string());
    let db_user = std::env::var("POSTGRES_USER").expect("No env POSTGRES_USER was provided");
    let db_pass = std::env::var("POSTGRES_PASSWORD").expect("No env POSTGRES_PASSWORD was provided");
    let db_name = std::env::var("POSTGRES_DB").unwrap_or("tuidb".to_string());

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        db_user, db_pass, db_host, db_port, db_name
    );
    println!("{}",database_url);

    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
