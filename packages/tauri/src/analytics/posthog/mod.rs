pub mod mock;
pub mod real;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;

#[async_trait]
pub trait Client {
    async fn capture(&self, event: Event) -> Result<(), Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Connection error: {0}")]
    Connection(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct Event {
    event: String,
    properties: Properties,
    timestamp: Option<NaiveDateTime>,
}

#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
pub struct Properties {
    distinct_id: String,
    props: HashMap<String, serde_json::Value>,
}

impl Properties {
    fn new<S: Into<String>>(distinct_id: S) -> Self {
        Self {
            distinct_id: distinct_id.into(),
            props: Default::default(),
        }
    }

    pub fn insert<K: Into<String>, P: Serialize>(&mut self, key: K, prop: P) -> Result<(), Error> {
        let as_json = serde_json::to_value(prop)?;
        let _ = self.props.insert(key.into(), as_json);
        Ok(())
    }
}

impl Event {
    pub fn new<S: Into<String>>(event: S, distinct_id: S) -> Self {
        Self {
            event: event.into(),
            properties: Properties::new(distinct_id),
            timestamp: None,
        }
    }

    /// Errors if `prop` fails to serialize
    pub fn insert_prop<K: Into<String>, P: Serialize>(
        &mut self,
        key: K,
        prop: P,
    ) -> Result<(), Error> {
        self.properties.insert(key, prop)?;
        Ok(())
    }
}
