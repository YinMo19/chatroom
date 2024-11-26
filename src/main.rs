#[macro_use]
extern crate rocket;

use rocket::form::Form;
use rocket::fs::{relative, FileServer};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::select;
use rocket::tokio::sync::broadcast::{channel, error::RecvError, Sender};
use rocket::{Shutdown, State};
use rocket_db_pools::sqlx::{self, FromRow};
use rocket_db_pools::{Connection, Database};

#[derive(Database)]
#[database("messages")]
struct MessageLog(sqlx::SqlitePool);

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, FromRow)]
#[cfg_attr(test, derive(PartialEq, UriDisplayQuery))]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
}

/// Returns an infinite stream of server-sent events. Each event is a message
/// pulled from a broadcast queue sent by the `post` handler.
#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };

            yield Event::json(&msg);
        }
    }
}

/// Receive a message from a form submission and broadcast it to any receivers.
#[post("/message", data = "<form>")]
async fn post(mut db: Connection<MessageLog>, form: Form<Message>, queue: &State<Sender<Message>>) {
    // A send 'fails' if there are no active subscribers. That's okay.

    let message = form.into_inner();
    let _res = queue.send(message.clone());

    let query = r#"
        INSERT INTO messages (room, username, message)
        VALUES (?, ?, ?)
    "#;

    let _result = sqlx::query(&query)
        .bind(&message.room)
        .bind(&message.username)
        .bind(&message.message)
        .fetch_all(&mut **db)
        .await
        .expect("Failed to insert message");
}

#[get("/history?<room>")]
async fn get_history(
    mut db: Connection<MessageLog>,
    room: String,
) -> Result<Json<Vec<Message>>, rocket::response::status::Custom<String>> {
    let query = r#"
        SELECT * FROM messages WHERE room = ? ORDER BY id DESC LIMIT 500
    "#;

    let mut messages = sqlx::query_as(&query)
        .bind(room)
        .fetch_all(&mut **db)
        .await
        .map_err(|e| {
            rocket::response::status::Custom(
                rocket::http::Status::InternalServerError,
                format!("Database error: {}", e),
            )
        })?;

    messages.reverse();
    Ok(Json(messages))
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let _rocket = rocket::build()
        .attach(MessageLog::init())
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, events, get_history])
        .mount("/", FileServer::from(relative!("static")))
        .launch()
        .await?;

    Ok(())
}
