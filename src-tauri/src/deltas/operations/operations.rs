use difference::{Changeset, Difference};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    // corresponds to YText.insert(index, chunk)
    Insert((u32, String)),
    // corresponds to YText.remove_range(index, len)
    Delete((u32, u32)),
}

impl Operation {
    pub fn apply(&self, text: &mut Vec<char>) {
        match self {
            Operation::Insert((index, chunk)) => {
                text.splice(*index as usize..*index as usize, chunk.chars());
            }
            Operation::Delete((index, len)) => {
                text.splice(*index as usize..(*index + *len) as usize, "".chars());
            }
        }
    }
}

pub fn get_delta_operations(initial_text: &str, final_text: &str) -> Vec<Operation> {
    if initial_text == final_text {
        return vec![];
    }

    let changeset = Changeset::new(initial_text, final_text, "");
    let mut offset: u32 = 0;
    let mut deltas = vec![];

    for edit in changeset.diffs {
        match edit {
            Difference::Rem(text) => {
                deltas.push(Operation::Delete((offset, text.len() as u32)));
            }
            Difference::Add(text) => {
                deltas.push(Operation::Insert((offset, text.to_string())));
                offset += text.len() as u32;
            }
            Difference::Same(text) => {
                offset += text.len() as u32;
            }
        }
    }

    return deltas;
}
