#[macro_use]
extern crate rocket;

mod routes;
mod models;
mod database;
// mod utils;

use models::Message;
use database::init_message_database;
use rocket_db_pools::Database;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    init_message_database().await;

    let _rocket = rocket::build()
        .attach(database::MessageLog::init())
        .manage(rocket::tokio::sync::broadcast::channel::<Message>(1024).0)
        .mount("/", routes![routes::post, routes::events, routes::get_history])
        .mount("/", rocket::fs::FileServer::from("static"))
        .launch()
        .await?;

    Ok(())
}