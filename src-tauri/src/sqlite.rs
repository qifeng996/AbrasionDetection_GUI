use rusqlite::{Connection, params};
pub fn connect_to_db()->Result<String,String>{
    match Connection::open("sqlite.db") {
        Ok(_Conn) => {
            Ok("Connection successful".to_string())
        }
        Err(_) => Err(format!("Unable to connect to database: {}", "sqlite.db")),
    }
}