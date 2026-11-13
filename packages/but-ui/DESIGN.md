# Design notes

The visual language of every GitButler app, on every platform: the React apps on
`@gitbutler/but-ui-react`, the GPUI client and anything built next. Enough to
make an on-brand choice without opening Figma. The rules hold for any app; the
examples are GitButler's own surfaces. What one app decides for itself lives in
its own design notes.

The Figma library is 🍑 But UI, the library for every app. 💎 Core is a
different library, for the Svelte desktop app. How the tokens themselves are
made (the ramps, the grounds' tint, the accent, the settings as data) is in this
package's README (`packages/but-ui/README.md`).

The notes are one page per topic, in `design/` beside this file. Read the pages
for what you are building, not all of them; each stands on its own.

## Getting started

| Page                                                                                       | Read it when                                                     |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- |
| [Build from the library](../but-ui-react/design/getting-started/build-from-the-library.md) | Starting any UI in a React app, or the library seems to lack one |

## Foundations

| Page                                             | Read it when                                         |
| ------------------------------------------------ | ---------------------------------------------------- |
| [Emphasis](design/foundations/emphasis.md)       | Choosing which control stands out, and in what color |
| [States](design/foundations/states.md)           | Anything clickable: its hover and focus              |
| [Cursors](design/foundations/cursors.md)         | Anything clickable, or a drag                        |
| [Minimums](design/foundations/minimums.md)       | A small control or small text                        |
| [Radius](design/foundations/radius.md)           | A corner, especially one inside another              |
| [Spacing](design/foundations/spacing.md)         | Room between cards, blocks or controls               |
| [Line breaks](design/foundations/line-breaks.md) | Copy that wraps                                      |
| [Motion](design/foundations/motion.md)           | A transition or an animation                         |
| [Icons](design/foundations/icons.md)             | An icon, a file glyph, or a person's picture         |

## Content

| Page                             | Read it when        |
| -------------------------------- | ------------------- |
| [Voice](design/content/voice.md) | Any words on screen |

## Components

Rules for one component that its docs don't carry yet.

| Page                                                | Read it when                                  |
| --------------------------------------------------- | --------------------------------------------- |
| [Buttons](design/components/buttons.md)             | Picking a `Button` variant or size            |
| [Badges](design/components/badges.md)               | Picking a `Badge`'s colour                    |
| [Banners](design/components/banners.md)             | A condition that holds until it clears        |
| [Meta lines](design/components/meta-lines.md)       | The facts under a header's title, or in a row |
| [View headers](design/components/view-headers.md)   | A details view's header: its buttons and tabs |
| [Location bars](design/components/location-bars.md) | The bar above a view's header                 |
| [Links](design/components/links.md)                 | A link, or text that opens something          |
| [Tooltips](design/components/tooltips.md)           | A hover hint                                  |
| [Fields](design/components/fields.md)               | A form, a settings row, a search box          |
| [Sidebar trees](design/components/sidebar-trees.md) | The sidebar's rows, how they fold and nest    |

## Patterns

| Page                                                            | Read it when                                     |
| --------------------------------------------------------------- | ------------------------------------------------ |
| [Modals](design/patterns/modals.md)                             | A prompt, a confirmation or a short form         |
| [Empty states](design/patterns/empty-states.md)                 | Nothing to show yet, or a filter matched nothing |
| [Blocked states](design/patterns/blocked-states.md)             | Something the user can't do yet                  |
| [Toasts and snackbars](design/patterns/toasts-and-snackbars.md) | Telling the user something happened              |
| [Markdown](design/patterns/markdown.md)                         | Rendering a description or a comment             |
