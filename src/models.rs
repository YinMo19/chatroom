use chrono::NaiveDateTime;
use rocket::form::{FromFormField, ValueField};
use rocket::http::uri::fmt::{Formatter, Query, UriDisplay};
use rocket::serde::{Deserialize, Serialize};
use rocket_db_pools::sqlx::{self, FromRow};
use rocket_db_pools::Database;
use sqlx::sqlite::{SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite, Type};
use std::fmt::{self, Write};


#[derive(Database)]
#[database("messages")]
pub struct MessageLog(sqlx::SqlitePool);

#[derive(Debug, Clone, FromForm, Serialize, Deserialize, FromRow)]
#[cfg_attr(test, derive(PartialEq, UriDisplayQuery))]
#[serde(crate = "rocket::serde")]
pub struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message: String,
    pub created_at: DateTimeWrapper,
    #[field(validate = len(..16))]
    pub ip_addr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DateTimeWrapper(String);

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
