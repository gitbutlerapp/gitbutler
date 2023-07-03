use std::{fmt::Display, ops::RangeInclusive};

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Eq, Clone)]
pub struct Hunk {
    hash: Option<String>,
    start: usize,
    end: usize,
}

impl PartialEq for Hunk {
    fn eq(&self, other: &Self) -> bool {
        if self.hash.is_some() && other.hash.is_some() {
            self.hash == other.hash
        } else {
            self.start == other.start && self.end == other.end
        }
    }
}

impl From<RangeInclusive<usize>> for Hunk {
    fn from(range: RangeInclusive<usize>) -> Self {
        Hunk {
            start: *range.start(),
            end: *range.end(),
            hash: None,
        }
    }
}

impl TryFrom<&str> for Hunk {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut range = s.split('-');
        let start = if let Some(raw_start) = range.next() {
            raw_start
                .parse::<usize>()
                .context(format!("failed to parse start of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;

        let end = if let Some(raw_end) = range.next() {
            raw_end
                .parse::<usize>()
                .context(format!("failed to parse end of range: {}", s))
        } else {
            Err(anyhow!("invalid range: {}", s))
        }?;

        let hash = range.next().map(|s| s.to_string());

        Hunk::new(start, end, hash)
    }
}

impl Display for Hunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(hash) = self.hash.as_ref() {
            write!(f, "{}-{}-{}", self.start, self.end, hash)
        } else {
            write!(f, "{}-{}", self.start, self.end)
        }
    }
}

impl Hunk {
    pub fn new(start: usize, end: usize, hash: Option<String>) -> Result<Self> {
        if start > end {
            Err(anyhow!("invalid range: {}-{}", start, end))
        } else {
            Ok(Hunk { start, end, hash })
        }
    }

    pub fn contains(&self, line: &usize) -> bool {
        self.start <= *line && self.end >= *line
    }

    pub fn intersects(&self, another: &Hunk) -> bool {
        self.contains(&another.start)
            || self.contains(&another.end)
            || another.contains(&self.start)
            || another.contains(&self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_from_string() {
        let hunk = Hunk::try_from("1-2").unwrap();
        assert_eq!("1-2", hunk.to_string());
    }

    #[test]
    fn parse_invalid() {
        assert!(Hunk::try_from("3-2").is_err());
    }

    #[test]
    fn parse_with_hash() {
        assert_eq!(
            Hunk::try_from("2-3-hash").unwrap(),
            Hunk::new(2, 3, Some("hash".to_string())).unwrap()
        );
    }

    #[test]
    fn parse_invalid_2() {
        assert!(Hunk::try_from("3-2").is_err());
    }

    #[test]
    fn test_eq() {
        vec![
            (
                Hunk::try_from("1-2").unwrap(),
                Hunk::try_from("1-2").unwrap(),
                true,
            ),
            (
                Hunk::try_from("1-2").unwrap(),
                Hunk::try_from("2-3").unwrap(),
                false,
            ),
            (
                Hunk::try_from("1-2-abc").unwrap(),
                Hunk::try_from("1-2-abc").unwrap(),
                true,
            ),
            (
                Hunk::try_from("1-2-abc").unwrap(),
                Hunk::try_from("2-3-abc").unwrap(),
                true,
            ),
            (
                Hunk::try_from("1-2").unwrap(),
                Hunk::try_from("1-2-abc").unwrap(),
                true,
            ),
            (
                Hunk::try_from("1-2-abc").unwrap(),
                Hunk::try_from("1-2").unwrap(),
                true,
            ),
            (
                Hunk::try_from("1-2-abc").unwrap(),
                Hunk::try_from("1-2-bcd").unwrap(),
                false,
            ),
            (
                Hunk::try_from("1-2-abc").unwrap(),
                Hunk::try_from("2-3-bcd").unwrap(),
                false,
            ),
        ]
        .iter()
        .for_each(|(a, b, expected)| {
            assert_eq!(a == b, *expected, "comapring {} and {}", a, b);
        });
    }
}
