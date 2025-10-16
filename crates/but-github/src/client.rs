use anyhow::Result;
use but_secret::Sensitive;
use octorust::{Client, auth::Credentials, types::UsersGetByUsernameResponseOneOf};
use serde::{Deserialize, Serialize};

pub struct GitHubClient {
    github: Client,
}

impl GitHubClient {
    pub fn new(access_token: &Sensitive<String>) -> Result<Self> {
        let github = Client::new(
            String::from("gb-github-integration"),
            Credentials::Token(access_token.to_string()),
        )?;

        Ok(Self { github })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        self.github
            .users()
            .get_authenticated()
            .await
            .map(|response| match response.body {
                UsersGetByUsernameResponseOneOf::PrivateUser(user) => {
                    let name = (!user.name.is_empty()).then(|| user.name.clone());
                    let email = (!user.email.is_empty()).then(|| user.email.clone());
                    let avatar_url = (!user.avatar_url.is_empty()).then(|| user.avatar_url.clone());
                    AuthenticatedUser {
                        login: user.login,
                        avatar_url,
                        name,
                        email,
                    }
                }
                UsersGetByUsernameResponseOneOf::PublicUser(user) => {
                    let name = (!user.name.is_empty()).then(|| user.name.clone());
                    let email = (!user.email.is_empty()).then(|| user.email.clone());
                    let avatar_url = (!user.avatar_url.is_empty()).then(|| user.avatar_url.clone());
                    AuthenticatedUser {
                        login: user.login,
                        avatar_url,
                        name,
                        email,
                    }
                }
            })
            .map_err(Into::into)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthenticatedUser {
    pub login: String,
    pub avatar_url: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}
