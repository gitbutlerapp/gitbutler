use crate::users;

pub struct Signature<'a> {
    signature: git2::Signature<'a>,
}

impl Clone for Signature<'static> {
    fn clone(&self) -> Self {
        Self {
            signature: self.signature.clone(),
        }
    }
}

impl<'a> From<Signature<'a>> for git2::Signature<'a> {
    fn from(value: Signature<'a>) -> Self {
        value.signature
    }
}

impl<'a> From<&'a Signature<'a>> for &'a git2::Signature<'a> {
    fn from(value: &'a Signature<'a>) -> Self {
        &value.signature
    }
}

impl<'a> From<git2::Signature<'a>> for Signature<'a> {
    fn from(value: git2::Signature<'a>) -> Self {
        Self { signature: value }
    }
}

impl TryFrom<&users::User> for Signature<'_> {
    type Error = super::Error;

    fn try_from(value: &users::User) -> Result<Self, Self::Error> {
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
}

impl Signature<'_> {
    pub fn now(name: &str, email: &str) -> Result<Self, super::Error> {
        git2::Signature::now(name, email)
            .map(Into::into)
            .map_err(Into::into)
    }

    pub fn name(&self) -> Option<&str> {
        self.signature.name()
    }

    pub fn email(&self) -> Option<&str> {
        self.signature.email()
    }
}
