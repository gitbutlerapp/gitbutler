use bstr::{BString, ByteSlice as _, ByteVec as _};

pub struct CommitMessage {
    pub title: BString,
    pub body: BString,
    pub trailers: Vec<(BString, BString)>,
}

impl CommitMessage {
    pub fn to_bstring(&self) -> BString {
        let mut out = BString::default();
        out.push_str(self.title.clone());
        out.push_str(b"\n\n");
        out.push_str(self.body.clone());
        out.push_str(b"\n\n");
        out.push_str(self.trailers_as_bstring());
        out
    }

    fn trailers_as_bstring(&self) -> BString {
        let mut out = BString::default();
        for (index, trailer) in self.trailers.iter().enumerate() {
            let trailer = gix::bstr::join(": ", [&trailer.0, &trailer.1]);
            out.push_str(trailer);

            if index != self.trailers.len() - 1 {
                out.push_str(b"\n")
            }
        }
        out
    }

    pub fn new(commit: gix::objs::CommitRef<'_>) -> Self {
        let message_ref = commit.message();
        let body_ref = message_ref.body();

        CommitMessage {
            title: commit.message().title.to_owned(),
            body: body_ref
                .map(|body_ref| body_ref.without_trailer().as_bstr().to_owned())
                .unwrap_or_default(),
            trailers: body_ref
                .map(|body_ref| {
                    body_ref
                        .trailers()
                        .map(|trailer| (trailer.token.to_owned(), trailer.value.to_owned()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        }
    }
}
