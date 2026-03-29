use rusqlite::Connection;
use std::path::PathBuf;

fn main() {
    let db_path = PathBuf::from("C:\\Users\\abdur\\AppData\\Local\\opencode-rtk\\history.db");
    if !db_path.exists() {
        println!("Database not found at {:?}", db_path);
        return;
    }

    let conn = Connection::open(&db_path).unwrap();
    let mut stmt = conn
        .prepare("SELECT id, command, saved_tokens FROM commands ORDER BY id DESC LIMIT 5")
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0).unwrap();
            let command: String = row.get(1).unwrap();
            let saved_tokens: i64 = row.get(2).unwrap();
            Ok((id, command, saved_tokens))
        })
        .unwrap();

    println!("Last 5 commands:");
    for row in rows {
        let (id, command, saved_tokens) = row.unwrap();
        println!(
            "ID: {}, Command: {}, Saved Tokens: {}",
            id, command, saved_tokens
        );
    }
}
