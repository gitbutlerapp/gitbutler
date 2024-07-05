use rusqlite::Connection;

// TODO: rename to patches
/// The changes struct provides a
pub struct Changes<'l> {
    connection: &'l mut Connection,
}

impl<'l> Changes<'l> {}
