use std::path;

use anyhow::{Context, Result};
use similar::TextDiff;

#[derive(Debug, PartialEq, Clone)]
pub enum DiffLine {
    Equal {
        old_index: usize,
        new_index: usize,
        line: String,
    },
    Delete {
        old_index: usize,
        line: String,
    },
    Insert {
        new_index: usize,
        line: String,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub struct Hunk {
    pub description: String,
    pub diff: Vec<DiffLine>,
}

impl Hunk {
    pub fn old_start_lines(&self) -> (usize, usize) {
        let numbers = self.diff.iter().filter_map(|diff_line| match diff_line {
            DiffLine::Equal { old_index, .. } => Some(old_index),
            DiffLine::Delete { old_index, .. } => Some(old_index),
            DiffLine::Insert { .. } => None,
        });
        let min = numbers.clone().min().unwrap();
        let max = numbers.max().unwrap();
        (*min, max - min)
    }

    pub fn new_start_lines(&self) -> (usize, usize) {
        let numbers = self.diff.iter().filter_map(|diff_line| match diff_line {
            DiffLine::Equal { new_index, .. } => Some(new_index),
            DiffLine::Delete { .. } => None,
            DiffLine::Insert { new_index, .. } => Some(new_index),
        });
        let min = numbers.clone().min().unwrap();
        let max = numbers.max().unwrap();
        (*min, max - min)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FileDiff {
    pub old_file_path: path::PathBuf,
    pub new_file_path: path::PathBuf,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn to_patch(&self) -> String {
        let mut lines = vec![];

        lines.push(format!("--- {}", self.old_file_path.display()));
        lines.push(format!("+++ {}", self.new_file_path.display()));

        for hunk in &self.hunks {
            let (old_start, old_lines) = hunk.old_start_lines();
            let (new_start, new_lines) = hunk.new_start_lines();
            lines.push(format!(
                "@@ -{},{} +{},{} @@ {}",
                old_start, old_lines, new_start, new_lines, hunk.description,
            ));
            for diff_line in &hunk.diff {
                match &diff_line {
                    DiffLine::Equal { line, .. } => lines.push(format!(" {}", line)),
                    DiffLine::Delete { line, .. } => lines.push(format!("-{}", line)),
                    DiffLine::Insert { line, .. } => lines.push(format!("+{}", line)),
                }
            }
        }

        lines.join("\n")
    }

    pub fn from_patch(patch: &str) -> Result<Self> {
        let mut lines_iter = patch.lines();
        let old_file_path = lines_iter
            .next()
            .context("patch is empty")?
            .strip_prefix("--- ")
            .context("patch is missing old file path")?;
        let new_file_path = lines_iter
            .next()
            .context("patch is missing new file path")?
            .strip_prefix("+++ ")
            .context("patch is missing new file path")?;

        let mut hunks: Vec<Hunk> = vec![];
        let mut old_line_number = 0;
        let mut new_line_number = 0;
        for line in lines_iter {
            match line {
                line if line.starts_with("@@") => {
                    let mut line_iter = line.chars();
                    let mut line_iter = line_iter.by_ref().skip(4); // '@@ -'
                    let old_start = line_iter
                        .by_ref()
                        .take_while(|c| c.is_numeric())
                        .collect::<String>()
                        .parse::<usize>()
                        .context("invalid old range start")?;
                    let _old_lines = line_iter
                        .by_ref()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<usize>()
                        .context("invalid old range end")?;
                    let mut line_iter = line_iter.by_ref().skip(1); // '+'
                    let new_start = line_iter
                        .by_ref()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<usize>()
                        .context("invalid new range start")?;
                    let _new_lines = line_iter
                        .by_ref()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<usize>()
                        .context("invalid new range end")?;
                    let line_iter = line_iter.by_ref().skip(3); // ' @@'
                    let description = line_iter.collect::<String>();
                    old_line_number = old_start;
                    new_line_number = new_start;
                    hunks.push(Hunk {
                        description,
                        diff: vec![],
                    });
                }
                line if line.starts_with(' ') => {
                    hunks.last_mut().unwrap().diff.push(DiffLine::Equal {
                        old_index: old_line_number,
                        new_index: new_line_number,
                        line: line[1..].to_string(),
                    });
                    old_line_number += 1;
                    new_line_number += 1;
                }
                line if line.starts_with('-') => {
                    hunks.last_mut().unwrap().diff.push(DiffLin);
                    old_line_number += 1;
                }
                line if line.starts_with('+') => {
                    hunks.last_mut().unwrap().diff.push(DiffLine {
                        line_number: new_line_number,
                        diff: DiffLine::Insert(line[1..].to_string()),
                    });
                    new_line_number += 1;
                }
                _ => return Err(anyhow::anyhow!("invalid diff line: {}", line)),
            }
        }

        Ok(Self {
            old_file_path: old_file_path.into(),
            new_file_path: new_file_path.into(),
            hunks,
        })
    }
}

pub fn diff(left: &str, right: &str, context: usize) -> Vec<Hunk> {
    let diff = TextDiff::configure().diff_lines(left, right);
    let mut hunks = vec![];
    for group in diff.grouped_ops(context) {
        let mut diff = vec![];
        println!("{:?}", group);
        for op in group {
            match op {
                similar::DiffOp::Equal {
                    new_index,
                    old_index,
                    len,
                    ..
                } => {
                    for index in 0..len {
                        let n = new_index + index;
                        let o = old_index + index;
                        diff.push(DiffLine::Equal {
                            old_index: o,
                            new_index: n,
                            line: left.lines().nth(o).unwrap().into(),
                        });
                    }
                }
                similar::DiffOp::Delete {
                    old_index,
                    old_len,
                    new_index,
                } => {
                    for index in old_index..old_index + old_len {
                        diff.push(DiffLine::Delete {
                            old_index: index,
                            line: left.lines().nth(index).unwrap().into(),
                        })
                    }
                }
                similar::DiffOp::Insert {
                    new_index, new_len, ..
                } => {
                    for index in new_index..new_index + new_len {
                        diff.push(DiffLine::Insert {
                            new_index: index,
                            line: right.lines().nth(index).unwrap().into(),
                        })
                    }
                }
                similar::DiffOp::Replace {
                    old_index,
                    old_len,
                    new_index,
                    new_len,
                } => {
                    for index in old_index..old_index + old_len {
                        diff.push(DiffLine::Delete {
                            old_index: index,
                            line: left.lines().nth(index).unwrap().into(),
                        });
                    }
                    for index in new_index..new_index + new_len {
                        diff.push(DiffLine::Insert {
                            new_index: index,
                            line: right.lines().nth(index).unwrap().into(),
                        });
                    }
                }
            }
        }
        hunks.push(Hunk {
            description: "".into(),
            diff,
        });
    }
    hunks
}

mod tests {
    use super::*;

    #[test]
    fn test_diff_two_hunks() {
        let left = vec!["b", "c", "c", "d", "e"];
        let right = vec!["a", "b", "c", "c", "d"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![
                Hunk {
                    description: "".into(),
                    diff: vec![
                        DiffLine {
                            line_number: 0,
                            diff: DiffLine::Insert("a".into()),
                        },
                        DiffLine {
                            line_number: 1,
                            diff: DiffLine::Equal("b".into()),
                        },
                    ]
                },
                Hunk {
                    description: "".into(),
                    diff: vec![
                        DiffLine {
                            line_number: 3,
                            diff: DiffLine::Equal("c".into()),
                        },
                        DiffLine {
                            line_number: 3,
                            diff: DiffLine::Delete("d".into()),
                        },
                        DiffLine {
                            line_number: 4,
                            diff: DiffLine::Delete("e".into()),
                        },
                        DiffLine {
                            line_number: 4,
                            diff: DiffLine::Insert("d".into()),
                        },
                    ]
                }
            ]
        );
    }

    #[test]
    fn test_diff_start_delete() {
        let left = vec!["a", "b", "c", "d", "e"];
        let right = vec!["b", "c", "d", "e"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 0,
                        diff: DiffLine::Delete("a".into()),
                    },
                    DiffLine {
                        line_number: 0,
                        diff: DiffLine::Equal("b".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_diff_middle_delete() {
        let left = vec!["a", "b", "c", "d", "e"];
        let right = vec!["a", "b", "d", "e"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 1,
                        diff: DiffLine::Equal("b".into()),
                    },
                    DiffLine {
                        line_number: 2,
                        diff: DiffLine::Delete("c".into()),
                    },
                    DiffLine {
                        line_number: 2,
                        diff: DiffLine::Equal("d".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_diff_end_delete() {
        let left = vec!["a", "b", "c", "d", "e"];
        let right = vec!["a", "b", "c", "d"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 2,
                        diff: DiffLine::Equal("c".into()),
                    },
                    DiffLine {
                        line_number: 3,
                        diff: DiffLine::Delete("d".into()),
                    },
                    DiffLine {
                        line_number: 4,
                        diff: DiffLine::Delete("e".into()),
                    },
                    DiffLine {
                        line_number: 3,
                        diff: DiffLine::Insert("d".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_diff_start_insert() {
        let left = vec!["b", "c", "d", "e"];
        let right = vec!["a", "b", "c", "d", "e"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 0,
                        diff: DiffLine::Insert("a".into()),
                    },
                    DiffLine {
                        line_number: 1,
                        diff: DiffLine::Equal("b".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_diff_middle_insert() {
        let left = vec!["a", "b", "d", "e"];
        let right = vec!["a", "b", "c", "d", "e"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 1,
                        diff: DiffLine::Equal("b".into()),
                    },
                    DiffLine {
                        line_number: 2,
                        diff: DiffLine::Insert("c".into()),
                    },
                    DiffLine {
                        line_number: 3,
                        diff: DiffLine::Equal("d".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_diff_end_insert() {
        let left = vec!["a", "b", "c", "d"];
        let right = vec!["a", "b", "c", "d", "e"];
        assert_eq!(
            diff(&left.join("\n"), &right.join("\n"), 1),
            vec![Hunk {
                description: "".into(),
                diff: vec![
                    DiffLine {
                        line_number: 2,
                        diff: DiffLine::Equal("c".into()),
                    },
                    DiffLine {
                        line_number: 3,
                        diff: DiffLine::Delete("d".into()),
                    },
                    DiffLine {
                        line_number: 3,
                        diff: DiffLine::Insert("d".into()),
                    },
                    DiffLine {
                        line_number: 4,
                        diff: DiffLine::Insert("e".into()),
                    },
                ]
            }]
        );
    }

    #[test]
    fn test_to_from() {
        let file_diff = FileDiff {
            old_file_path: "old_file_path".into(),
            new_file_path: "new_file_path".into(),
            hunks: vec![Hunk {
                description: "description".into(),
                diff: vec![
                    DiffLine {
                        line_number: 11,
                        diff: DiffLine::Equal("line1".into()),
                    },
                    DiffLine {
                        line_number: 12,
                        diff: DiffLine::Delete("line2".into()),
                    },
                    DiffLine {
                        line_number: 12,
                        diff: DiffLine::Insert("line3".into()),
                    },
                ],
            }],
        };

        let patch = file_diff.to_patch();
        assert_eq!(file_diff, FileDiff::from_patch(&patch).unwrap());
    }
}
