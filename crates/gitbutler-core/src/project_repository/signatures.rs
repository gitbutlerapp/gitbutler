use crate::users;
use anyhow::Result;

pub fn signatures<'a>(
    project_repository: &super::Repository,
    user: Option<&users::User>,
) -> Result<(git2::Signature<'a>, git2::Signature<'a>)> {
    let config = project_repository.config();

    let author = match (user, config.user_name()?, config.user_email()?) {
        (_, Some(name), Some(email)) => git2::Signature::now(&name, &email)?,
        (Some(user), _, _) => try_from(user)?,
        _ => git2::Signature::now("GitButler", "gitbutler@gitbutler.com")?,
    };

    let comitter = if config.user_real_comitter()? {
        author.clone()
    } else {
        git2::Signature::now("GitButler", "gitbutler@gitbutler.com")?
    };

    Ok((author, comitter))
}

fn try_from(value: &users::User) -> Result<git2::Signature<'static>> {
    let name = value
        .name
        .as_deref()
        .or(value.given_name.as_deref())
        .unwrap_or(&value.email);
    Ok(git2::Signature::now(name, &value.email)?)
}
