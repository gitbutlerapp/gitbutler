#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialsKeyOption {
    BringYourOwn,
    #[serde(rename = "butlerAPI")]
    ButlerApi,
}

impl CredentialsKeyOption {
    pub fn from_git_config_value(s: &str) -> Option<Self> {
        match s {
            "bringYourOwn" => Some(CredentialsKeyOption::BringYourOwn),
            "butlerAPI" => Some(CredentialsKeyOption::ButlerApi),
            _ => None,
        }
    }

    pub fn as_git_config_value(self) -> &'static str {
        match self {
            CredentialsKeyOption::BringYourOwn => "bringYourOwn",
            CredentialsKeyOption::ButlerApi => "butlerAPI",
        }
    }
}
