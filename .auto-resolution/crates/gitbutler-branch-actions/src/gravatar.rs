use anyhow::Result;

pub fn gravatar_url_from_email(email: &str) -> Result<url::Url> {
    let gravatar_url = format!(
        "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
        md5::compute(email.to_lowercase())
    );
    url::Url::parse(gravatar_url.as_str()).map_err(Into::into)
}
