mod reader;
mod writer;

pub use reader::BranchReader as Reader;
pub use writer::BranchWriter as Writer;

#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub upstream: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    pub tree: git2::Oid, // last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub ownership: Vec<String>,
}

impl TryFrom<&dyn crate::reader::Reader> for Branch {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let id = reader.read_string("id").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("id: {}", e),
            ))
        })?;
        let name = reader.read_string("meta/name").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/name: {}", e),
            ))
        })?;
        let applied = reader.read_bool("meta/applied").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/applied: {}", e),
            ))
        })?;
        let upstream = reader.read_string("meta/upstream").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/upstream: {}", e),
            ))
        })?;
        let tree = reader.read_string("meta/tree").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/tree: {}", e),
            ))
        })?;
        let created_timestamp_ms = reader.read_u128("meta/created_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/created_timestamp_ms: {}", e),
            ))
        })?;
        let updated_timestamp_ms = reader.read_u128("meta/updated_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/updated_timestamp_ms: {}", e),
            ))
        })?;
        let ownership_string = reader.read_string("meta/ownership").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/tree: {}", e),
            ))
        })?;
        // convert ownership string to Vec<String>
        let ownership = ownership_string
            .split('\n')
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        Ok(Self {
            id,
            name,
            applied,
            upstream,
            tree: git2::Oid::from_str(&tree).unwrap(),
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
        })
    }
}
