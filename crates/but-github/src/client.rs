use anyhow::Result;
use octorust::{Client, auth::Credentials, types::UsersGetByUsernameResponseOneOf};
use serde::{Deserialize, Serialize};

pub struct GitHubClient {
    github: Client,
}

impl GitHubClient {
    pub fn new(access_token: &str) -> Result<Self> {
        let github = Client::new(
            String::from("gb-github-integration"),
            Credentials::Token(String::from(access_token)),
        )?;

        Ok(Self { github })
    }

    pub async fn get_authenticated(&self) -> Result<AuthenticatedUser> {
        self.github
            .users()
            .get_authenticated()
            .await
            .map(|response| match response.body {
                UsersGetByUsernameResponseOneOf::PrivateUser(user) => AuthenticatedUser {
                    login: user.login,
                    name: user.name,
                    email: user.email,
                },
                UsersGetByUsernameResponseOneOf::PublicUser(user) => AuthenticatedUser {
                    login: user.login,
                    name: user.name,
                    email: user.email,
                },
            })
            .map_err(Into::into)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AuthenticatedUser {
    pub login: String,
    pub name: String,
    pub email: String,
}
