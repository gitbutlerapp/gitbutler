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

pub mod clock;
pub mod file_icons;
pub mod illustration;
pub mod modal;
#[expect(
    clippy::disallowed_methods,
    reason = "where colours are made, from but-ui's tokens or a colour scheme"
)]
pub mod theme;

mod avatar;
mod badge;
mod button;
mod checkbox;
mod diff_stats;
mod empty_state;
mod file_list;
mod file_status_badge;
mod focus;
mod fonts;
mod icon;
mod keys;
mod layout;
mod list_row;
mod machine_status;
mod meta_count;
mod popup;
mod radio;
mod segmented;
mod select;
mod sidebar_row;
mod skeleton_bone;
mod snackbar;
mod split_preview;
mod switch;
mod switch_button;
mod text;
mod toggle_group;
mod tooltip;

pub use avatar::*;
pub use badge::*;
pub use button::*;
pub use checkbox::*;
pub use diff_stats::*;
pub use empty_state::*;
pub use file_list::*;
pub use file_status_badge::*;
pub use focus::*;
pub use fonts::*;
pub use icon::*;
pub use keys::*;
pub use layout::*;
pub use list_row::*;
pub use machine_status::*;
pub use meta_count::*;
pub use popup::*;
pub use radio::*;
pub use segmented::*;
pub use select::*;
pub use sidebar_row::*;
pub use skeleton_bone::*;
pub use snackbar::*;
pub use split_preview::*;
pub use switch::*;
pub use switch_button::*;
pub use text::*;
pub use toggle_group::*;
pub use tooltip::*;
