//! THE PRESENCE RULE — one authority for whether an applied branch belongs to the
//! display, consumed by all three sites that need the answer:
//!
//! * the leftover-branch surface check ([`surface`](Presence::surface) — should an
//!   undisplayed branch be EMITTED as an empty segment at its rest?),
//! * the out-of-cone prune ([`demanded`](Presence::demanded) — may a prune pass
//!   DROP the branch's empty segment?),
//! * the display-completeness assert ([`demanded`](Presence::demanded) — must the
//!   projection SHOW the branch?).
//!
//! Letting them answer separately is a real bug class, not a tidiness concern: builder-fuzz
//! seed 0 vanishes an applied branch outright when the prune and the assert disagree about
//! integrated layout-empties, with chain position (who wears the base stamp) deciding the
//! outcome. The rule table below is the single place the policy lives; a site that needs a
//! different answer must change the TABLE, visibly for every other consumer.

/// Where a branch's resting commit sits relative to the workspace. Precedence
/// mirrors the completeness assert's original classification: integrated wins over
/// workspace membership (an unpulled target tip is Integrated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Territory {
    /// No resting commit at all (an unborn or positionless ref).
    Positionless,
    /// The rest is not a node of this graph.
    Vanished,
    /// The rest is target-reachable history.
    Integrated,
    /// The rest is live workspace territory (per the caller's membership test).
    InWorkspace,
    /// Anything else — unincorporated outside content.
    Outside,
}

impl Territory {
    /// Classify `rest`. `in_workspace` decides workspace membership for a live,
    /// non-integrated node — the surface check answers with the arena's `InWorkspace`
    /// flag, the completeness assert with tip REACHABILITY. That difference is a
    /// deliberate, open divergence (a below-bound rest in a target-less workspace
    /// classifies InWorkspace under reachability but Outside under the flag);
    /// closing it needs corpus evidence, not fiat — until then each caller states
    /// its test here instead of hiding it in a private classifier.
    pub(crate) fn of(
        cg: &crate::CommitGraph,
        rest: Option<gix::ObjectId>,
        in_workspace: impl Fn(gix::ObjectId) -> bool,
    ) -> Self {
        let Some(on) = rest else {
            return Territory::Positionless;
        };
        let Some(node) = cg.node(on) else {
            return Territory::Vanished;
        };
        if node.flags.contains(crate::CommitFlags::Integrated) {
            Territory::Integrated
        } else if in_workspace(on) {
            Territory::InWorkspace
        } else {
            Territory::Outside
        }
    }
}

/// The rule's two answers. `surface` is the surfacing question, `demanded` the
/// prune's and the assert's; they differ deliberately — a positionless or vanished
/// rest SURFACES (the branch has nowhere else to appear) without being DEMANDED
/// (the assert tolerates its absence, so display machinery keeps some slack there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Presence {
    /// Emit the branch as a leftover empty segment at its rest when no stack
    /// already displays it.
    pub surface: bool,
    /// The display must keep the branch: prunes may not drop it, and the
    /// completeness assert fails if it is missing.
    pub demanded: bool,
}

/// THE TABLE — presence of an APPLIED, non-archived, non-`gitbutler/*` branch that
/// names a segment, by the territory of its rest:
///
/// | territory                    | surface | demanded            |
/// |------------------------------|---------|---------------------|
/// | Positionless                 | yes     | no                  |
/// | Vanished                     | yes     | no                  |
/// | Integrated                   | yes     | layout-empty or at-bound |
/// | InWorkspace, rest displayed  | no (carried) | yes            |
/// | InWorkspace, rest undisplayed| yes     | yes                 |
/// | Outside                      | no      | no                  |
///
/// Row rationale, kept from the arcs that pinned each:
/// * Integrated demands only LAYOUT-empties (or a rest ON the bound): a
///   display-emptied shell of an integrated commit-bearing branch may hide (the
///   upstream-advanced leak class), a true leftover empty may not (seed 0).
/// * Outside never surfaces: adopting outside content is materialize's
///   reconciliation work, never display fiat (consolidation 28's contract —
///   mid-operation projections feed the merges apply builds).
/// * A displayed InWorkspace rest is CARRIED — the branch rides or splices into
///   the stack that shows the commit; emitting it again would duplicate it.
pub(crate) fn leftover_presence(
    territory: Territory,
    layout_empty: bool,
    at_bound: bool,
    rest_displayed: bool,
) -> Presence {
    match territory {
        Territory::Positionless | Territory::Vanished => Presence {
            surface: true,
            demanded: false,
        },
        Territory::Integrated => Presence {
            surface: true,
            demanded: layout_empty || at_bound,
        },
        Territory::InWorkspace => Presence {
            surface: !rest_displayed,
            demanded: true,
        },
        Territory::Outside => Presence {
            surface: false,
            demanded: false,
        },
    }
}
