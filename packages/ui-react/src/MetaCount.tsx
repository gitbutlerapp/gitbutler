import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./MetaCount.module.css";

/** What a {@link MetaCount} counts, which sets its glyph and its colour. */
export type MetaCountType =
	| "behind"
	| "unpushed"
	| "commits"
	| "empty"
	| "head"
	| "uncommitted"
	| "clean"
	| "age"
	| "branch"
	| "worktrees"
	| "repos"
	| "agent-working"
	| "agent-waiting";

const glyph: Record<MetaCountType, IconName> = {
	behind: "arrow-down",
	unpushed: "arrow-up",
	commits: "commit",
	empty: "commit",
	head: "branch",
	uncommitted: "diff",
	clean: "diff",
	age: "clock",
	branch: "branch",
	worktrees: "folder",
	repos: "repo",
	"agent-working": "robot",
	"agent-waiting": "robot",
};

/**
 * A glyph and a short value in a row's meta: a count, an age, a branch's name. The type sets both.
 * Amber is work waiting on you (`behind`, `uncommitted`, `agent-waiting`), blue is work to send
 * (`unpushed`), green is an agent at work (`agent-working`); the rest are neutral. Quietest of all
 * are `empty`, a branch with no commits yet, and `head`, what a worktree with nothing to list is on
 * (`empty`, `detached`); a row shows either first, ahead of its counts.
 *
 * The value is the children, so a host formats it (`12`, `3d`, `retry-transfers`). The glyph says
 * nothing to a screen reader: wrap the count in a `Tooltip` saying what it counts, or give it an
 * `aria-label`. A long value, such as a branch's name, ends in an ellipsis once its row runs out
 * of room.
 *
 * @import import { MetaCount } from "@gitbutler/ui-react/MetaCount.tsx";
 */
export const MetaCount: FC<{ type: MetaCountType } & ComponentProps<"span">> = ({
	type,
	children,
	...props
}) => (
	<span {...props} className={classes(props.className, "text-13", styles.count, styles[type])}>
		<Icon name={glyph[type]} size={12} className={styles.glyph} />
		<span className={styles.value}>{children}</span>
	</span>
);

/** The state of a branch's checks, as {@link CiStatus} shows it. */
export type CiStatusType = "working" | "passed" | "failed";

const ciGlyph: Record<CiStatusType, IconName> = {
	working: "spinner",
	passed: "tick",
	failed: "cross",
};

const ciLabel: Record<CiStatusType, string> = {
	working: "Checks running",
	passed: "Checks passed",
	failed: "Checks failed",
};

/**
 * The state of a branch's pull request checks, as "CI" in amber while they run, green once they
 * pass and red if one fails. It sits in a row's meta beside the {@link MetaCount}s.
 *
 * @import import { CiStatus } from "@gitbutler/ui-react/MetaCount.tsx";
 */
export const CiStatus: FC<{ status: CiStatusType } & ComponentProps<"span">> = ({
	status,
	...props
}) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph and a word with one name, not a picture with a source.
		role="img"
		aria-label={ciLabel[status]}
		{...props}
		className={classes(props.className, "text-13", styles.count, styles[status])}
	>
		<Icon name={ciGlyph[status]} size={12} className={styles.glyph} />
		<span className={styles.value}>CI</span>
	</span>
);
