pub struct Blob<'a>(git2::Blob<'a>);

impl<'a> From<git2::Blob<'a>> for Blob<'a> {
    fn from(value: git2::Blob<'a>) -> Self {
        Self(value)
    }
}

impl Blob<'_> {
    pub fn content(&self) -> &[u8] {
        self.0.content()
    }

    pub fn size(&self) -> usize {
        self.0.size()
    }
}
