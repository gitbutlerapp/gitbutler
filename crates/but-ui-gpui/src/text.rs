//! Text in the type scale, as `@gitbutler/but-ui`'s text classes and Figma's text styles name it:
//! `.text_13()` is `text-13`, Figma's `Base/13/regular`, and `.text_13().semibold()` is
//! `text-13 text-semibold`, `Base/13/semibold`.
//!
//! Each size method sets the class's size, weight and line height, as the class does, so a
//! modifier comes after it. The values are generated from `packages/but-ui/styles/text.css` and
//! the font tokens in `tokens.css`, so the scale is changed there, never here.

use std::ops::Range;

use gpui::{
    FontWeight, HighlightStyle, SharedString, Styled, StyledText, UnderlineStyle, relative,
};

use crate::{dp, fixed};

/// One of but-ui's base text classes: its size in px as designed, its weight as but-ui writes it
/// (`--but-font-weight-regular`, 460) and its line height as a share of the size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextClass {
    pub size: f32,
    pub weight: f32,
    pub line_height: f32,
}

include!(concat!(env!("OUT_DIR"), "/text_tokens.rs"));

/// The embedded Inter file that draws but-ui's `weight`. Each file is cut lighter than the weight
/// it stands in for, to match but.dev's stems (`assets/fonts/README.md`), and GPUI picks a file by
/// its named weight: Regular for regular, SemiBold for semibold, and bold, which has no file of
/// its own, from SemiBold made bold.
pub fn weight(weight: f32) -> FontWeight {
    if weight >= FONT_WEIGHT_BOLD {
        FontWeight::BOLD
    } else if weight >= FONT_WEIGHT_SEMIBOLD {
        FontWeight::SEMIBOLD
    } else {
        FontWeight::NORMAL
    }
}

/// The text classes on any element, as `div().text_13().semibold()`. Clippy holds the rest of
/// both crates to them: `text_size` and `font_weight` are set here alone.
pub trait Text: Styled + Sized {
    /// A base class: its size, weight and line height.
    #[expect(
        clippy::disallowed_methods,
        reason = "the classes are where text gets its size and weight"
    )]
    fn text_class(self, class: TextClass) -> Self {
        self.text_size(dp(class.size))
            .font_weight(weight(class.weight))
            .line_height(relative(class.line_height))
    }

    fn text_11(self) -> Self {
        self.text_class(TEXT_11)
    }

    fn text_12(self) -> Self {
        self.text_class(TEXT_12)
    }

    fn text_13(self) -> Self {
        self.text_class(TEXT_13)
    }

    fn text_14(self) -> Self {
        self.text_class(TEXT_14)
    }

    fn text_15(self) -> Self {
        self.text_class(TEXT_15)
    }

    fn text_16(self) -> Self {
        self.text_class(TEXT_16)
    }

    /// A Markdown heading, as Figma's Markdown/H1 to H3.
    fn text_h1(self) -> Self {
        self.text_class(TEXT_H1)
    }

    fn text_h2(self) -> Self {
        self.text_class(TEXT_H2)
    }

    fn text_h3(self) -> Self {
        self.text_class(TEXT_H3)
    }

    /// A size off the scale, `px` as designed, for text sized to something other than the type
    /// scale: a letter filling an avatar or a key, code at the size the user set. Its line height
    /// and weight are left as they are.
    #[expect(
        clippy::disallowed_methods,
        reason = "the classes are where text gets its size and weight"
    )]
    fn text_off_scale(self, px: f32) -> Self {
        self.text_size(dp(px))
    }

    /// `text-semibold`, after a size.
    #[expect(
        clippy::disallowed_methods,
        reason = "the classes are where text gets its size and weight"
    )]
    fn semibold(self) -> Self {
        self.font_weight(weight(FONT_WEIGHT_SEMIBOLD))
    }

    /// `text-bold`, after a size.
    #[expect(
        clippy::disallowed_methods,
        reason = "the classes are where text gets its size and weight"
    )]
    fn bold(self) -> Self {
        self.font_weight(weight(FONT_WEIGHT_BOLD))
    }

    /// `text-body`, after a size: the line height for reading, as Figma's Body styles.
    fn body(self) -> Self {
        self.line_height(relative(LINE_HEIGHT_BODY))
    }
}

impl<T: Styled> Text for T {}

/// `text` with the letters a search found in it underlined, so a list shows why each row is
/// there. `found` are byte ranges of `text`, in order and apart.
pub fn matched(
    text: impl Into<SharedString>,
    found: impl IntoIterator<Item = Range<usize>>,
) -> StyledText {
    let underline = HighlightStyle {
        underline: Some(UnderlineStyle {
            thickness: fixed(1.),
            ..Default::default()
        }),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(found.into_iter().map(|range| (range, underline)))
}

#[cfg(test)]
mod tests {
    use gpui::FontWeight;

    use super::*;

    #[test]
    fn the_scale_reads_from_but_uis_classes() {
        assert_eq!(
            TEXT_13,
            TextClass {
                size: 13.,
                weight: FONT_WEIGHT_REGULAR,
                line_height: LINE_HEIGHT_DEFAULT,
            },
            "text-13 is 13px, regular, on the default line, as text.css says"
        );
        assert_eq!(
            (TEXT_11.size, TEXT_16.size, TEXT_H1.size),
            (11., 16., 20.),
            "the base sizes and the headings, from their rem"
        );
        assert_eq!(
            (FONT_WEIGHT_REGULAR, FONT_WEIGHT_SEMIBOLD, FONT_WEIGHT_BOLD),
            (460., 580., 700.),
            "the weights, from tokens.css"
        );
        assert_eq!(
            (LINE_HEIGHT_DEFAULT, LINE_HEIGHT_BODY),
            (1.2, 1.6),
            "line heights, from their percentages"
        );
    }

    #[test]
    fn each_weight_draws_with_the_file_that_stands_in_for_it() {
        assert_eq!(weight(FONT_WEIGHT_REGULAR), FontWeight::NORMAL, "regular");
        assert_eq!(
            weight(FONT_WEIGHT_SEMIBOLD),
            FontWeight::SEMIBOLD,
            "semibold"
        );
        assert_eq!(weight(FONT_WEIGHT_BOLD), FontWeight::BOLD, "bold");
        assert_eq!(
            weight(TEXT_H1.weight),
            FontWeight::SEMIBOLD,
            "a heading is semibold"
        );
    }
}
