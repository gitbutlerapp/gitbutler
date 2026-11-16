//! GitButler's design system on GPUI: the theme, read from but-ui's tokens, and the components
//! that have a counterpart in `@gitbutler/but-ui-react`.
//!
//! Each component's module names its counterparts in its first doc paragraph, as
//! "as but-ui-react's `Button`": the docs site reads that to mark those pages GPUI.
// Built into the GPUI client, where a panic quits GitButler.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::allow_attributes,
    clippy::dbg_macro,
    clippy::let_underscore_must_use,
    clippy::undocumented_unsafe_blocks
)]
#![cfg_attr(not(test), deny(clippy::indexing_slicing, clippy::string_slice))]
#![cfg_attr(test, allow(clippy::disallowed_methods))]

pub mod illustration;
pub mod modal;
#[expect(
    clippy::disallowed_methods,
    reason = "where colours are made, from but-ui's tokens or a colour scheme"
)]
pub mod theme;

mod badge;
mod button;
mod checkbox;
mod empty_state;
mod focus;
mod fonts;
mod icon;
mod keys;
mod layout;
mod list_row;
mod meta_count;
mod popup;
mod segmented;
mod select;
mod snackbar;
mod switch;
mod text;
mod toggle_group;
mod tooltip;

pub use badge::*;
pub use button::*;
pub use checkbox::*;
pub use empty_state::*;
pub use focus::*;
pub use fonts::*;
pub use icon::*;
pub use keys::*;
pub use layout::*;
pub use list_row::*;
pub use meta_count::*;
pub use popup::*;
pub use segmented::*;
pub use select::*;
pub use snackbar::*;
pub use switch::*;
pub use text::*;
pub use toggle_group::*;
pub use tooltip::*;
