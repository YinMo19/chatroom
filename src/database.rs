use rocket_db_pools::sqlx::{self, migrate::MigrateDatabase};
use rocket_db_pools::Database;
use sqlx::{Sqlite, SqlitePool};

const DB_URL: &str = "sqlite://messages.db";

#[derive(Database)]
#[database("messages")]
pub struct MessageLog(sqlx::SqlitePool);

pub async fn init_message_database() {
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
