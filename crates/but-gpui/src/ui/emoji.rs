//! What a project can wear as its icon, as but.dev's repository picker offers it: an emoji,
//! what a repository is about first, then a shared set, each found by its name and keywords.
//! Without one it wears the repo glyph.

use std::sync::LazyLock;

use gpui::{AnyElement, IntoElement as _, Styled as _, div, prelude::*};

use super::{Icon, icon_at};
use crate::theme::Theme;

/// What a project's stored icon is, as but.dev reads it: an emoji, or nothing, which is the
/// repo glyph. but.dev also stored a colour of that glyph (`glyph:<colour>`, and `shape:<color>`
/// before it); the colours are gone, so those wear the glyph as no icon does.
pub enum ProjectIcon<'a> {
    Glyph,
    Emoji(&'a str),
}

impl<'a> ProjectIcon<'a> {
    /// No icon, and a colour but.dev stored before, is the glyph.
    pub fn parse(stored: Option<&'a str>) -> Self {
        let Some(stored) = stored.filter(|s| !s.is_empty()) else {
            return ProjectIcon::Glyph;
        };
        // Every form but.dev stored the glyph in names a colour after a colon
        // (`glyph:teal`, and `square:blue` before it), and no emoji holds one.
        match stored.contains(':') {
            true => ProjectIcon::Glyph,
            false => ProjectIcon::Emoji(stored),
        }
    }

    /// The glyph `px` design pixels square, or the emoji in the text size around it.
    pub fn render(&self, px: f32, theme: &Theme) -> AnyElement {
        match self {
            ProjectIcon::Glyph => {
                (icon_at(Icon::Repo, px).text_color(theme.text_1)).into_any_element()
            }
            ProjectIcon::Emoji(emoji) => div().child(emoji.to_string()).into_any_element(),
        }
    }
}

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

/// The rest of the set, chosen to tell things apart at a glance, as but-ui-react's avatar picker
/// offered them before but.dev took the picker over.
const SHARED: [(&str, &str, &str); 84] = [
    ("🕹️", "joystick", "game arcade"),
    ("🐶", "dog", "puppy pet animal"),
    ("🐱", "cat", "kitten pet animal"),
    ("🐸", "frog", "animal green"),
    ("🐻", "bear", "animal"),
    ("🌻", "sunflower", "flower plant yellow"),
    ("🐙", "octopus", "animal sea"),
    ("🦄", "unicorn", "animal magic"),
    ("🐝", "bee", "insect animal honey"),
    ("🍕", "pizza", "food"),
    ("🌵", "cactus", "plant desert green"),
    ("🚀", "rocket", "space launch ship"),
    ("🎸", "guitar", "music instrument"),
    ("🍩", "doughnut", "donut food sweet"),
    ("🌈", "rainbow", "weather colour"),
    ("🔥", "fire", "flame hot"),
    ("🦖", "dinosaur", "t-rex animal"),
    ("🍉", "watermelon", "fruit food"),
    ("⚡", "lightning", "bolt zap electric fast"),
    ("🐧", "penguin", "animal bird linux"),
    ("🍋", "lemon", "fruit food yellow"),
    ("💻", "laptop", "computer machine mac"),
    ("🖥️", "desktop computer", "machine monitor screen"),
    ("📺", "television", "tv screen"),
    ("📱", "phone", "mobile device"),
    ("⌨️", "keyboard", "computer typing"),
    ("🤖", "robot", "bot agent ai machine"),
    ("👾", "alien monster", "game space invader"),
    ("🛰️", "satellite", "space orbit remote"),
    ("☁️", "cloud", "weather remote server"),
    ("🌙", "moon", "night crescent"),
    ("⭐", "star", "favourite"),
    ("🪐", "planet", "saturn space"),
    ("🌍", "globe", "earth world planet"),
    ("🏔️", "mountain", "snow peak"),
    ("🌊", "wave", "sea ocean water"),
    ("🍀", "clover", "luck plant green"),
    ("🌲", "tree", "evergreen forest plant"),
    ("🍄", "mushroom", "plant fungus"),
    ("🐢", "turtle", "animal slow"),
    ("🦊", "fox", "animal"),
    ("🐼", "panda", "animal bear"),
    ("🦉", "owl", "animal bird night"),
    ("🐳", "whale", "animal sea docker"),
    ("🦀", "crab", "animal sea rust"),
    ("🐞", "ladybug", "insect bug animal"),
    ("🦋", "butterfly", "insect animal"),
    ("🍓", "strawberry", "fruit food"),
    ("🥑", "avocado", "fruit food green"),
    ("🌮", "taco", "food"),
    ("☕", "coffee", "drink hot cup"),
    ("🍪", "cookie", "food sweet biscuit"),
    ("🎮", "game controller", "gamepad play"),
    ("🎲", "dice", "game random"),
    ("🧩", "puzzle", "piece jigsaw"),
    ("🎯", "target", "bullseye dart"),
    ("🎨", "palette", "art paint design"),
    ("📚", "books", "library read"),
    ("🔬", "microscope", "science lab"),
    ("🧪", "test tube", "science lab experiment"),
    ("🔭", "telescope", "space science"),
    ("⚙️", "gear", "settings cog machine"),
    ("🔧", "wrench", "tool fix spanner"),
    ("🧲", "magnet", "attract"),
    ("💡", "light bulb", "idea"),
    ("🔋", "battery", "power energy"),
    ("📡", "antenna", "satellite dish signal remote"),
    ("🗿", "moai", "statue stone"),
    ("🏰", "castle", "building"),
    ("🚲", "bicycle", "bike"),
    ("⛵", "sailboat", "boat sea"),
    ("✈️", "airplane", "plane travel flight"),
    ("🎈", "balloon", "party"),
    ("👻", "ghost", "spooky halloween"),
    ("💎", "gem", "diamond jewel"),
    ("🔮", "crystal ball", "magic"),
    ("🧭", "compass", "direction navigate"),
    ("🏆", "trophy", "win prize"),
    ("🎹", "piano", "music keyboard instrument"),
    ("🎧", "headphones", "music audio"),
    ("📷", "camera", "photo picture"),
    ("🌋", "volcano", "mountain eruption"),
    ("🍔", "burger", "hamburger food"),
    ("🐿️", "chipmunk", "squirrel animal"),
];

/// Every emoji to pick from, in the picker's order, each once.
fn all() -> &'static [Emoji] {
    static ALL: LazyLock<Vec<Emoji>> = LazyLock::new(|| {
        let mut all: Vec<Emoji> = Vec::new();
        let choices = REPOSITORY.iter().chain(&SHARED);
        for emoji in choices.map(|&(emoji, name, keywords)| Emoji {
            emoji,
            name,
            keywords,
        }) {
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
    use super::{ProjectIcon, all, search};

    #[test]
    fn shared_emoji_come_after_the_repository_ones() {
        assert!(all().len() > 60, "the shared set follows the repository's");
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

    #[test]
    fn reads_icons_as_but_dev_stores_them() {
        let key = |stored| match ProjectIcon::parse(stored) {
            ProjectIcon::Glyph => "glyph".to_owned(),
            ProjectIcon::Emoji(emoji) => emoji.to_owned(),
        };
        assert_eq!(key(None), "glyph", "no icon is the glyph");
        assert_eq!(key(Some("")), "glyph", "nor is an empty one");
        assert_eq!(
            key(Some("glyph:safe")),
            "glyph",
            "a colour but.dev stored is the glyph, now the colours are gone"
        );
        assert_eq!(
            key(Some("square:blue")),
            "glyph",
            "as are the shapes it stored before them"
        );
        assert_eq!(key(Some("🦀")), "🦀", "anything else is an emoji");
    }
}
