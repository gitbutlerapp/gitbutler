use crate::{git, users};

pub fn signatures<'a>(
    project_repository: &super::Repository,
    user: Option<&users::User>,
) -> Result<(git::Signature<'a>, git::Signature<'a>), git::Error> {
    let config = project_repository.config();

    let author = match (user, config.user_name()?, config.user_email()?) {
        (_, Some(name), Some(email)) => git::Signature::now(&name, &email)?,
        (Some(user), _, _) => git::Signature::try_from(user)?,
        _ => git::Signature::now("GitButler", "gitbutler@gitbutler.com")?,
    };

    let comitter = if config.user_real_comitter()? {
        author.clone()
    } else {
        git::Signature::now("GitButler", "gitbutler@gitbutler.com")?
    };

    Ok((author, comitter))
}
