//! The emoji a project can wear, as but.dev's repository picker offers them: what a
//! repository is about first, then ui-react's set, each found by its name and keywords.

use std::sync::LazyLock;

pub struct Emoji {
    pub emoji: &'static str,
    pub name: &'static str,
    pub keywords: &'static str,
}

/// What a repository is about, as but.dev's picker puts first, and a few for the
/// repositories that deserve them.
const REPOSITORY: [(&str, &str, &str); 21] = [
    ("📂", "folder", "files open directory"),
    ("📦", "package", "box library module"),
    ("🗄️", "file cabinet", "archive storage"),
    ("🧪", "test tube", "experiment lab test"),
    ("🎨", "palette", "design art ui"),
    ("🚀", "rocket", "space launch ship"),
    ("📖", "book", "docs read manual"),
    ("🔒", "lock", "private security"),
    ("⚡", "lightning", "bolt zap electric fast"),
    ("🐛", "bug", "insect fix"),
    ("📊", "chart", "data stats analytics"),
    ("🧹", "broom", "cleanup chore"),
    ("🔥", "fire", "flame hot"),
    ("🔑", "key", "auth secret"),
    ("🍆", "eggplant", "aubergine vegetable"),
    ("🍑", "peach", "fruit butt"),
    ("💦", "droplets", "sweat splash water"),
    ("💩", "poop", "poo pile crap"),
    ("🖕", "middle finger", "rude flip"),
    ("🤬", "cursing face", "swear angry symbols"),
    ("🤡", "clown", "circus joker"),
];

/// ui-react's choices, one to a line as `{ emoji: "🐶", name: "dog", keywords: "…" },`.
const UI_REACT: &str = include_str!("../../../../packages/ui-react/src/entityAvatarChoices.ts");

/// Every emoji to pick from, in the picker's order, each once.
pub fn all() -> &'static [Emoji] {
    static ALL: LazyLock<Vec<Emoji>> = LazyLock::new(|| {
        let repository = REPOSITORY.iter().map(|&(emoji, name, keywords)| Emoji {
            emoji,
            name,
            keywords,
        });
        let shared = UI_REACT.lines().filter_map(|line| {
            let rest = line.trim().strip_prefix("{ emoji: \"")?;
            let (emoji, rest) = rest.split_once("\", name: \"")?;
            let (name, rest) = rest.split_once("\", keywords: \"")?;
            let keywords = rest.strip_suffix("\" },")?;
            Some(Emoji {
                emoji,
                name,
                keywords,
            })
        });
        let mut all: Vec<Emoji> = Vec::new();
        for emoji in repository.chain(shared) {
            if !all.iter().any(|seen| seen.emoji == emoji.emoji) {
                all.push(emoji);
            }
        }
        all
    });
    &ALL
}

/// The emoji whose name or keywords hold every word of `query`.
pub fn search(query: &str) -> Vec<&'static Emoji> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    (all().iter())
        .filter(|emoji| {
            words.iter().all(|word| {
                emoji.name.contains(word.as_str()) || emoji.keywords.contains(word.as_str())
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{all, search};

    #[test]
    fn ui_reacts_emoji_are_read_after_the_repository_ones() {
        assert!(
            all().len() > 60,
            "ui-react's list reads, one to a line, as entityAvatarChoices.ts writes it"
        );
        assert_eq!(all()[0].emoji, "📂", "the repository's own come first");
        assert_eq!(
            search("pet animal")
                .iter()
                .map(|e| e.name)
                .collect::<Vec<_>>(),
            ["dog", "cat"],
            "every word of a search has to match"
        );
    }
}
