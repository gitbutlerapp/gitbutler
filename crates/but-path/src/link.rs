//! The `but://` links the GUI opens, written by `but gui` and read by the apps that open them.
//!
//! Both sides go through [`OpenLink`], so what one writes is what the other reads.

use std::path::PathBuf;

use crate::AppChannel;

/// A `but://open?path=…` link: open the project at `path`, adding it first if it's new.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenLink {
    /// The project's directory.
    pub path: PathBuf,
    /// Open it in a window of its own rather than the one in front.
    pub new_window: bool,
}

impl OpenLink {
    /// The link as `channel`'s app takes it. `timestamp` makes each link differ from the last,
    /// so the platform doesn't drop a repeat of one it just delivered.
    pub fn to_url(&self, channel: AppChannel, timestamp: u128) -> anyhow::Result<url::Url> {
        let mut url = url::Url::parse(&format!("{}://open", channel.scheme()))?;
        url.query_pairs_mut()
            .append_pair("path", &self.path.to_string_lossy())
            .append_pair("t", &timestamp.to_string())
            .append_pair("new_window", if self.new_window { "1" } else { "0" });
        Ok(url)
    }

    /// Read `url` as an open link of any channel's scheme, or `None` if it isn't one or names
    /// no path. Parameters it doesn't know are left alone, so newer links still open the
    /// project in older apps.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url::Url::parse(url).ok()?;
        let known_scheme = (AppChannel::ALL.iter()).any(|channel| channel.scheme() == url.scheme());
        if !known_scheme || url.host_str() != Some("open") {
            return None;
        }
        let mut path = None;
        let mut new_window = false;
        for (key, value) in url.query_pairs() {
            match &*key {
                "path" if !value.is_empty() => path = Some(PathBuf::from(&*value)),
                "new_window" => new_window = value == "1",
                _ => {}
            }
        }
        Some(OpenLink {
            path: path?,
            new_window,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::path::Path;

    use super::OpenLink;
    use crate::AppChannel;

    #[test]
    fn to_url_encodes_query_parameters() -> anyhow::Result<()> {
        let path = Path::new(r"C:\project & tools\#one");
        let link = OpenLink {
            path: path.to_owned(),
            new_window: true,
        };
        let url = link.to_url(AppChannel::Release, 123)?;

        assert_eq!(
            url.as_str(),
            "but://open?path=C%3A%5Cproject+%26+tools%5C%23one&t=123&new_window=1",
            "reserved path characters are encoded"
        );
        assert_eq!(url.scheme(), "but", "the requested app channel is retained");
        assert_eq!(url.host_str(), Some("open"), "the open command is retained");
        assert_eq!(
            url.query_pairs().collect::<HashMap<_, _>>(),
            HashMap::from([
                (Cow::Borrowed("path"), path.to_string_lossy()),
                (Cow::Borrowed("t"), Cow::Borrowed("123")),
                (Cow::Borrowed("new_window"), Cow::Borrowed("1")),
            ]),
            "query values round-trip after encoding"
        );
        Ok(())
    }

    #[test]
    fn parse_reads_what_to_url_writes() -> anyhow::Result<()> {
        for channel in AppChannel::ALL {
            for new_window in [false, true] {
                let link = OpenLink {
                    path: r"/Users/me/project & tools/#one".into(),
                    new_window,
                };
                let url = link.to_url(channel, 123)?;
                assert_eq!(
                    OpenLink::parse(url.as_str()),
                    Some(link),
                    "every channel's link reads back as written"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn parse_ignores_unknown_parameters() {
        assert_eq!(
            OpenLink::parse("but://open?path=%2Frepo&view=history&new_window=0"),
            Some(OpenLink {
                path: "/repo".into(),
                new_window: false,
            }),
            "parameters a newer app writes don't stop an older one opening the project"
        );
    }

    #[test]
    fn parse_rejects_other_links() {
        for url in [
            "https://open?path=%2Frepo",
            "but://login?access_token=x&t=1",
            "but://app/project/x/workspace",
            "but://open?t=1",
            "but://open?path=",
            "not a url",
        ] {
            assert_eq!(OpenLink::parse(url), None, "{url} is not an open link");
        }
    }
}
