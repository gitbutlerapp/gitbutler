use anyhow::Result;
use core::str;

pub struct CommitBuffer {
    heading: Vec<(String, String)>,
    message: String,
}

impl CommitBuffer {
    pub fn new(buffer: &[u8]) -> Result<Self> {
        let buffer = str::from_utf8(buffer)?;

        dbg!(&buffer);

        if let Some((heading, message)) = buffer.split_once("\n\n") {
            let heading = heading
                .lines()
                .filter_map(|line| line.split_once(' '))
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect();

            Ok(Self {
                heading,
                message: message.to_string(),
            })
        } else {
            Ok(Self {
                heading: vec![],
                message: buffer.to_string(),
            })
        }
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        let mut set_heading = false;
        self.heading.iter_mut().for_each(|(k, v)| {
            if k == key {
                *v = value.to_string();
                set_heading = true;
            }
        });

        if !set_heading {
            self.heading.push((key.to_string(), value.to_string()));
        }
    }

    pub fn set_change_id(&mut self, change_id: Option<&str>) {
        let change_id = change_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));

        self.set_header("change-id", change_id.as_str());
    }

    pub fn as_string(&self) -> String {
        let mut output = String::new();

        for (key, value) in &self.heading {
            output.push_str(&format!("{} {}\n", key, value));
        }

        output.push('\n');

        output.push_str(&self.message);

        output
    }
}

impl TryFrom<git2::Buf> for CommitBuffer {
    type Error = anyhow::Error;

    fn try_from(git2_buffer: git2::Buf) -> Result<Self> {
        Self::new(&git2_buffer)
    }
}

impl TryFrom<String> for CommitBuffer {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self> {
        Self::new(s.as_bytes())
    }
}

impl From<CommitBuffer> for String {
    fn from(buffer: CommitBuffer) -> String {
        buffer.as_string()
    }
}
