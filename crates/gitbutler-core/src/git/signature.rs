pub struct Signature<'a> {
    pub signature: git2::Signature<'a>,
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
