mod reader;
mod writer;

use serde::{ser::SerializeStruct, Serialize, Serializer};

pub use reader::TargetReader as Reader;
pub use writer::TargetWriter as Writer;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub name: String,
    pub remote: String,
    pub sha: git2::Oid,
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Target", 3)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("remote", &self.remote)?;
        state.serialize_field("sha", &self.sha.to_string())?;
        state.end()
    }
}

impl TryFrom<&dyn crate::reader::Reader> for Target {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let name = reader.read_string("name").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("name: {}", e),
            ))
        })?;
        let remote = reader.read_string("remote").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("remote: {}", e),
            ))
        })?;
        let sha = reader
            .read_string("sha")
            .map_err(|e| {
                crate::reader::Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("sha: {}", e),
                ))
            })?
            .parse()
            .map_err(|e| {
                crate::reader::Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("sha: {}", e),
                ))
            })?;

        Ok(Self { name, remote, sha })
    }
}
