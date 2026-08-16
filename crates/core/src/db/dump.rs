use crate::db::Library;

#[test]
fn dump_triggers() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let mut stmt = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE type = 'trigger'")
        .expect("prepare");
    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .expect("query")
        .map(Result::unwrap)
        .collect();
    for (name, sql) in rows {
        eprintln!("TRIGGER {name}:\n{sql}\n");
    }
    let mut stmt = conn
        .prepare("SELECT sql FROM sqlite_master WHERE name = 'track_fts'")
        .expect("prepare");
    let fts: Vec<String> = stmt
        .query_map([], |row| Ok(row.get::<_, String>(0)?))
        .expect("query")
        .map(Result::unwrap)
        .collect();
    for sql in fts {
        eprintln!("TRACK_FTS:\n{sql}\n");
    }
}
