#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialsKeyOption {
    BringYourOwn,
    #[serde(rename = "butlerAPI")]
    ButlerApi,
}

impl CredentialsKeyOption {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "bringYourOwn" => Some(CredentialsKeyOption::BringYourOwn),
            "butlerAPI" => Some(CredentialsKeyOption::ButlerApi),
            _ => None,
        }
    }
}
