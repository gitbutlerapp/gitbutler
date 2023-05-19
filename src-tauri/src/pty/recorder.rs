use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Type {
    Input,
    Output,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub timestamp: u128,
    #[serde(rename = "type")]
    pub typ: Type,
    pub bytes: Vec<u8>,
}
