//! What a project can wear as its icon, as but.dev's repository picker offers it: the repo
//! glyph in one of its colours, or an emoji, what a repository is about first, then a shared
//! set, each found by its name and keywords.

use std::sync::LazyLock;

use gpui::{AnyElement, IntoElement as _, Styled as _, div, img, prelude::*};

use super::{Icon, dp, icon_at};
use crate::theme::Theme;

/// The repo glyph in one of its colours, as but.dev's `RepoGlyph` draws it.
pub struct Colour {
    /// What but.dev stores after `glyph:`; the outline is stored as no icon at all.
    pub key: &'static str,
    pub name: &'static str,
    /// The filled glyph; `None` for the outline, drawn in the text's colour.
    pub fill: Option<Icon>,
}

impl Colour {
    const fn new(key: &'static str, name: &'static str, fill: Option<Icon>) -> Self {
        Self { key, name, fill }
    }

    /// The icon as but.dev stores it.
    pub fn stored(&self) -> Option<String> {
        self.fill.map(|_| format!("glyph:{}", self.key))
    }
}

/// The glyph's colours, in the order but.dev's picker offers them: the outline first.
pub const COLOURS: [Colour; 7] = [
    Colour::new("outline", "Outline", None),
    Colour::new("pop", "Teal", Some(Icon::RepoPop)),
    Colour::new("safe", "Green", Some(Icon::RepoSafe)),
    Colour::new("purple", "Purple", Some(Icon::RepoPurple)),
    Colour::new("danger", "Red", Some(Icon::RepoDanger)),
    Colour::new("warn", "Orange", Some(Icon::RepoWarn)),
    Colour::new("gray", "Gray", Some(Icon::RepoGray)),
];

/// The colours whose name holds every word of `query`, as but.dev finds them, by their
/// name and "colour color glyph".
pub fn search_colours(query: &str) -> Vec<&'static Colour> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    (COLOURS.iter())
        .filter(|colour| {
            let haystack = format!("{} colour color glyph", colour.name.to_lowercase());
            words.iter().all(|word| haystack.contains(word.as_str()))
        })
        .collect()
}

/// What a project's stored icon is, as but.dev reads it: `glyph:<colour>`; `shape:color`,
/// what but.dev stored before the glyph, in the nearest colour; or an emoji.
pub enum ProjectIcon<'a> {
    Glyph(&'static Colour),
    Emoji(&'a str),
}

impl<'a> ProjectIcon<'a> {
    /// No icon is the glyph's outline.
    pub fn parse(stored: Option<&'a str>) -> Self {
        let Some(stored) = stored.filter(|s| !s.is_empty()) else {
            return ProjectIcon::Glyph(&COLOURS[0]);
        };
        let (kind, colour) = stored.split_once(':').unwrap_or((stored, ""));
        let key = match (kind, colour) {
            ("glyph", colour) => Some(colour),
            (_, "red") => Some("danger"),
            (_, "orange" | "yellow") => Some("warn"),
            (_, "green") => Some("safe"),
            (_, "teal" | "blue") => Some("pop"),
            (_, "purple" | "pink") => Some("purple"),
            _ => None,
        };
        match key.and_then(|key| COLOURS.iter().find(|c| c.key == key)) {
            Some(colour) => ProjectIcon::Glyph(colour),
            None => ProjectIcon::Emoji(stored),
        }
    }

    /// The glyph `px` design pixels square, or the emoji in the text size around it.
    pub fn render(&self, px: f32, theme: &Theme) -> AnyElement {
        match self {
            ProjectIcon::Glyph(Colour { fill: None, .. }) => icon_at(Icon::Repo, px)
                .text_color(theme.text_1)
                .into_any_element(),
            ProjectIcon::Glyph(Colour {
                fill: Some(fill), ..
            }) => img(fill.path()).size(dp(px)).flex_none().into_any_element(),
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
    use super::{ProjectIcon, all, search, search_colours};

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
        assert_eq!(
            search_colours("colour").len(),
            7,
            "every colour is found as a colour"
        );
        assert_eq!(search_colours("dog").len(), 0, "no colour is a dog");
    }

    #[test]
    fn reads_icons_as_but_dev_stores_them() {
        let key = |stored| match ProjectIcon::parse(stored) {
            ProjectIcon::Glyph(colour) => colour.key.to_owned(),
            ProjectIcon::Emoji(emoji) => emoji.to_owned(),
        };
        assert_eq!(key(None), "outline", "no icon is the outline");
        assert_eq!(key(Some("")), "outline", "nor is an empty one");
        assert_eq!(key(Some("glyph:safe")), "safe", "a glyph in its colour");
        assert_eq!(
            key(Some("square:blue")),
            "pop",
            "the old shapes in the nearest colour"
        );
        assert_eq!(key(Some("🦀")), "🦀", "anything else is an emoji");
    }
}
