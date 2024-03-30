use gitbutler::database::Database;

use crate::shared::temp_dir;

#[test]
fn smoke() {
    let data_dir = temp_dir();
    let db = Database::open_in_directory(data_dir.path()).unwrap();
    db.transaction(|tx| {
        tx.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", [])
            .unwrap();
        tx.execute("INSERT INTO test (id) VALUES (1)", []).unwrap();
        let mut stmt = tx.prepare("SELECT id FROM test").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let row = rows.next().unwrap().unwrap();
        let id: i32 = row.get(0).unwrap();
        assert_eq!(id, 1_i32);
        Ok(())
    })
    .unwrap();
}
