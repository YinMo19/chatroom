#[macro_use]
extern crate rocket;

use chrono::NaiveDateTime;
use rocket::form::{Form, FromFormField, ValueField};
use rocket::fs::{relative, FileServer};
use rocket::http::uri::fmt::{Formatter, Query, UriDisplay};
use rocket::response::stream::{Event, EventStream};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio::select;
use rocket::tokio::sync::broadcast::{channel, error::RecvError, Sender};
use rocket::{Shutdown, State};
use rocket_db_pools::sqlx::{self, FromRow};
use rocket_db_pools::{Connection, Database};
use sqlx::sqlite::{SqliteTypeInfo, SqliteValueRef};
use sqlx::{migrate::MigrateDatabase, Sqlite};
use sqlx::{Decode, Encode, SqlitePool, Type};
use std::fmt::{self, Write};

const DB_URL: &str = "sqlite://messages.db";

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
    pub created_at: DateTimeWrapper,
}

// 创建一个新的包装类型
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
struct DateTimeWrapper(String);

impl<'v> FromFormField<'v> for DateTimeWrapper {
    fn from_value(field: ValueField<'v>) -> rocket::form::Result<'v, Self> {
        let naive_date_time = NaiveDateTime::parse_from_str(field.value, "%Y/%m/%d %H:%M:%S")
            .map_err(|_| rocket::form::Error::validation("invalid datetime"));

        Ok(DateTimeWrapper(
            naive_date_time?.format("%Y-%m-%d %H:%M:%S").to_string(),
        ))
    }
}

impl UriDisplay<Query> for DateTimeWrapper {
    fn fmt(&self, f: &mut Formatter<'_, Query>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Type<Sqlite> for DateTimeWrapper {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for DateTimeWrapper {
    fn decode(
        value: SqliteValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let s = <&str as Decode<Sqlite>>::decode(value)?;
        Ok(DateTimeWrapper(s.to_string()))
    }
}

impl Encode<'_, Sqlite> for DateTimeWrapper {
    fn encode_by_ref(
        &self,
        buf: &mut <Sqlite as sqlx::database::HasArguments>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull {
        <String as Encode<Sqlite>>::encode(self.0.clone(), buf)
    }
}

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

#[post("/message", data = "<form>")]
async fn post(mut db: Connection<MessageLog>, form: Form<Message>, queue: &State<Sender<Message>>) {
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

async fn init_message_database() {
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    }

    let init_sql = r#"
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            room TEXT NOT NULL CHECK(length(room) <= 30),
            username TEXT NOT NULL CHECK(length(username) <= 20),
            message TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
    "#;

    let db = SqlitePool::connect(DB_URL).await.unwrap();
    let _result = sqlx::query(&init_sql).execute(&db).await.unwrap();
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    init_message_database().await;

    let _rocket = rocket::build()
        .attach(MessageLog::init())
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, events, get_history])
        .mount("/", FileServer::from(relative!("static")))
        .launch()
        .await?;

    Ok(())
}
