use std::{fs, io, path::Path, str};

use anyhow::{bail, Result};
use serde::{ser::SerializeStruct, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    UTF8(String),
    Binary,
    Large,
}

impl Serialize for Content {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Content::UTF8(text) => {
                let mut state = serializer.serialize_struct("Content", 2)?;
                state.serialize_field("type", "utf8")?;
                state.serialize_field("value", text)?;
                state.end()
            }
            Content::Binary => {
                let mut state = serializer.serialize_struct("Content", 1)?;
                state.serialize_field("type", "binary")?;
                state.end()
            }
            Content::Large => {
                let mut state = serializer.serialize_struct("Content", 1)?;
                state.serialize_field("type", "large")?;
                state.end()
            }
        }
    }
}

impl Content {
    const MAX_SIZE: usize = 1024 * 1024 * 10; // 10 MB

    pub fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)?;
        if metadata.len() > Content::MAX_SIZE as u64 {
            return Ok(Content::Large);
        }
        let content = fs::read(path)?;
        Ok(content.as_slice().into())
    }
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        if text.len() > Self::MAX_SIZE {
            Content::Large
        } else {
            Content::UTF8(text.to_string())
        }
    }
}

impl From<&[u8]> for Content {
    fn from(bytes: &[u8]) -> Self {
        if bytes.len() > Self::MAX_SIZE {
            Content::Large
        } else {
            match String::from_utf8(bytes.to_vec()) {
                Err(_) => Content::Binary,
                Ok(text) => Content::UTF8(text),
            }
        }
    }
}

impl TryFrom<&Content> for usize {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => text.parse().map_err(Into::into),
            Content::Binary => bail!("file is binary"),
            Content::Large => bail!("file too large"),
        }
    }
}

impl TryFrom<Content> for usize {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for String {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        match content {
            Content::UTF8(text) => Ok(text.clone()),
            Content::Binary => bail!("file is binary"),
            Content::Large => bail!("file too large"),
        }
    }
}

impl TryFrom<Content> for String {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<Content> for i64 {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for i64 {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(Into::into)
    }
}

impl TryFrom<Content> for u64 {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for u64 {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(Into::into)
    }
}

impl TryFrom<Content> for u128 {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for u128 {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(Into::into)
    }
}

impl TryFrom<Content> for bool {
    type Error = anyhow::Error;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Self::try_from(&content)
    }
}

impl TryFrom<&Content> for bool {
    type Error = anyhow::Error;

    fn try_from(content: &Content) -> Result<Self, Self::Error> {
        let text: String = content.try_into()?;
        text.parse().map_err(Into::into)
    }
}
