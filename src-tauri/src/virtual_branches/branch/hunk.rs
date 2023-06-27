use std::{fmt::Display, ops::RangeInclusive};

use anyhow::{anyhow, Context, Result};

static CONTEXT: usize = 3; // default git diff context

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Hunk {
    start: usize,
    end: usize,
}

impl From<RangeInclusive<usize>> for Hunk {
    fn from(range: RangeInclusive<usize>) -> Self {
        Hunk {
            start: *range.start(),
            end: *range.end(),
        }
    }
}

impl TryFrom<&str> for Hunk {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut range = s.split('-');
        if range.clone().count() != 2 {
            return Err(anyhow!("invalid range: {}", s));
        }
        let start = range
            .next()
            .unwrap()
            .parse::<usize>()
            .context(format!("failed to parse start of range: {}", s))?;
        let end = range
            .next()
            .unwrap()
            .parse::<usize>()
            .context(format!("failed to parse end of range: {}", s))?;
        if start > end {
            Err(anyhow!("invalid range: {}", s))
        } else {
            Ok(Hunk { start, end })
        }
    }
}

impl Display for Hunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end,)
    }
}

impl Hunk {
    pub fn start(&self) -> &usize {
        &self.start
    }

    pub fn contains(&self, line: &usize) -> bool {
        self.start <= *line && self.end >= *line
    }

    pub fn distance(&self, another: &Hunk) -> usize {
        if self.start > another.end {
            self.start - another.end
        } else if another.start > self.end {
            another.start - self.end
        } else {
            0
        }
    }

    pub fn touches(&self, another: &Hunk) -> bool {
        self.distance(another) <= CONTEXT * 2
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
        let hunk = Hunk::from(1..=2);
        assert_eq!(hunk, Hunk::try_from(hunk.to_string().as_str()).unwrap());
    }
}
