# Motion

**Two speeds, both tokens.** Every duration comes from design-core.
`--transition-fast` (80ms) is for a control changing state in place: hover and
press on a button, a focused field's outline, a small opacity fade.
`--transition-medium` (150ms) is for something that moves or changes shape: a
switch thumb, a chevron turning, a section folding, the minimap fading in. No
hand-written durations; if neither tier fits, add a tier in Figma.

**Popups are the medium tier with a curve.** `--transition-popup` aliases
medium, and `--easing-popup` is the one tokenised curve: a hard ease-out that
lands without overshoot, so a modal, dropdown or popover arrives rather than
drifts in. They always go together —
`transform var(--transition-popup) var(--easing-popup)` — and a modal's
backdrop, a sibling that can't inherit, takes the same pair. Popups close the
way they open.

**Easings are keywords.** Outside popups nothing names a curve: the tiers ride
the browser's default `ease`, and a place that wants another shape writes
`ease-out` after the duration. Don't tokenise `ease`; it would export as a
longer cubic-bezier that says less. In Figma, `ease` is a custom bezier of
0.25, 0.1, 0.25, 1; Ease in, Ease out and Ease in and out match the CSS
keywords of the same name.

**Feel comes from the curve before the tier.** A medium transition that seems
slow wants `ease-out`, not the fast tier.

**Loops and holds are not transitions.** The spinner is a keyframe animation
with its own timing; the pause before a "Copied" label reverts is a delay in
code. Neither takes a token.

**Anything that moves respects reduced motion.** A fold that changes height
turns its transition off under `prefers-reduced-motion: reduce`. A hover color
needs no such rule.

**An icon that becomes another icon crossfades.** Copy becoming a tick, plus
becoming a check, a placeholder becoming a camera under the pointer: both icons
stay in the DOM, one over the other (a shared grid cell or an absolutely
positioned wrapper), each transitioning `opacity, scale, filter` on the medium
tier with `ease-out`. The leaving one shrinks to `scale(0.25)`, fades to `0`
and blurs to `4px`; the arriving one does the reverse. It is a transition,
not a keyframe, so it reverses cleanly mid-swap, for result and hover swaps
alike.

**The rules live in two places.** The token descriptions in ⚛️ Core carry the
same tiers and pairings; change one and change the other.
