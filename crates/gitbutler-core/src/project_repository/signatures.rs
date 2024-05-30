use crate::{git, users};

pub fn signatures<'a>(
    project_repository: &super::Repository,
    user: Option<&users::User>,
) -> Result<(git2::Signature<'a>, git2::Signature<'a>), git::Error> {
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

fn try_from<'a>(value: &users::User) -> Result<git2::Signature<'a>, git::Error> {
    if let Some(name) = &value.name {
        git2::Signature::now(name, &value.email)
            .map(Into::into)
            .map_err(Into::into)
    } else if let Some(name) = &value.given_name {
        git2::Signature::now(name, &value.email)
            .map(Into::into)
            .map_err(Into::into)
    } else {
        git2::Signature::now(&value.email, &value.email)
            .map(Into::into)
            .map_err(Into::into)
    }
}
