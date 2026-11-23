import type { ComponentProps, FC } from "react";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./MetaCount.module.css";

/** What a {@link MetaCount} counts, which sets its glyph and its colour. */
export type MetaCountType =
	| "behind"
	| "conflicts"
	| "unpushed"
	| "rebased"
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
	| "agent-waiting"
	| "running"
	| "stack"
	| "cpu"
	| "cpu-danger";

const glyph: Record<MetaCountType, IconName> = {
	behind: "arrow-down",
	conflicts: "warning",
	unpushed: "arrow-up",
	rebased: "arrow-up",
	commits: "commit",
	empty: "commit",
	head: "anchor",
	uncommitted: "diff",
	clean: "diff",
	age: "clock",
	branch: "branch",
	worktrees: "folder",
	repos: "repo",
	"agent-working": "robot",
	"agent-waiting": "robot",
	running: "spinner",
	stack: "stack",
	cpu: "cpu",
	"cpu-danger": "cpu-danger",
};

type MetaCountProps = { type: MetaCountType } & ComponentProps<"span">;

/**
 * A glyph and a short value in a row's meta: a count, an age, a branch's name. The type sets both.
 * Amber is work waiting on you (`behind`, `uncommitted`, `agent-waiting`, and `conflicts`, how
 * many of a branch's commits are conflicted: a conflict waits on you, it hasn't failed), blue is
 * work to send
 * (`unpushed`); the rest are neutral. An agent at work (`agent-working`) is among them: it needs
 * nothing from you, and its robot moving says it's at work. So is `running`, any other program at
 * work in a terminal, as the terminal's own dots spinner, and `stack`: where a branch sits in its
 * stack, "1/3 in stack". `rebased` is the unpushed arrow in that neutral grey: commits that were
 * pushed and have been rewritten since, most often by a rebase onto the target, so a push replaces
 * them rather than sending new work. Quietest of all are `empty`, a branch with no commits yet, and `head`, what a worktree with nothing to list is on
 * (`empty`, `detached`); a row shows either first, ahead of its counts. `cpu` is a process's CPU
 * use while it stays low, in green; `cpu-danger` is the same use once it runs high, in red with a
 * warning on the chip.
 *
 * The value is the children, so a host formats it (`12`, `3d`, `retry-transfers`). The glyph says
 * nothing to a screen reader: wrap the count in a `Tooltip` saying what it counts, or give it an
 * `aria-label`. A long value, such as a branch's name, ends in an ellipsis once its row runs out
 * of room.
 *
 * A count is a 20px box with 3px at its sides, so counts sit flush in a row, 6px apart from glyph to
 * glyph, and a host puts no gap between them.
 *
 * A count is never pressed: it says how much, and the step that acts on it is a small `Button` in
 * the count's colour beside the counts, or in its place, as an `info` Push is for ↑ `unpushed` and
 * a `warning` Update for ↓ `behind`.
 *
 * @figma `value` turns the value off, as children left out do, for a robot or the running dots alone.
 * `running` draws the dots' first step.
 * @see packages/but-ui/design/components/meta-lines.md
 * @see packages/but-ui/design/patterns/work-under-way.md
 * @import import { MetaCount } from "@gitbutler/but-ui-react/MetaCount.tsx";
 */
export const MetaCount: FC<MetaCountProps> = ({ type, children, ...props }) => (
	<span
		{...props}
		className={classes(props.className, "text-13", styles.count, styles[type])}
		// A name, which the row it's in lets give way before the row's own name.
		data-meta-name={type === "head" ? "" : undefined}
		// A mark with no edge of its own, which a row sets in further when it ends on one.
		data-meta-count=""
	>
		{type === "agent-working" ? (
			<WorkingRobot />
		) : type === "running" ? (
			<RunningDots />
		) : (
			<Icon name={glyph[type]} size={14} className={classes(styles.glyph, styles.faded)} />
		)}
		{children !== undefined && <span className={styles.value}>{children}</span>}
	</span>
);

/** Where each of the six dots of {@link RunningDots} sits: two columns of three, top down. */
const runningDots = [0, 1, 2, 3, 4, 5].map((bit) => ({
	bit,
	cx: 5.25 + Math.floor(bit / 3) * 3.5,
	cy: 3.5 + (bit % 3) * 3.5,
}));

/**
 * A program at work in a terminal, as the terminal's own spinner draws it: six dots, braille's,
 * stepping round ten times a second. Drawn here rather than as an icon, as its dots change from
 * step to step; it holds still under reduced motion.
 */
const RunningDots: FC = () => (
	<svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true" className={styles.glyph}>
		{runningDots.map(({ bit, cx, cy }) => (
			<circle
				key={bit}
				className={classes(styles["spin-dot"], styles[`spin-dot-${bit}`])}
				cx={cx}
				cy={cy}
				r="1"
				fill="currentColor"
			/>
		))}
	</svg>
);

/**
 * The robot of an agent at work, on a loop: after a second of its face, its eyes become two bars
 * that wave out of step while the smile draws in, then settle back to the face. Drawn here rather
 * than as an icon, as its parts move apart; it holds still under reduced motion.
 */
const WorkingRobot: FC = () => (
	<svg
		width="14"
		height="14"
		viewBox="0 0 12 12"
		fill="none"
		aria-hidden="true"
		className={classes(styles.glyph, styles.faded)}
	>
		<rect x="1.5" y="1.5" width="9" height="9" stroke="currentColor" strokeWidth="1.5" />
		<rect x="0" y="3.75" width="1.5" height="3.75" fill="currentColor" />
		<rect x="10.5" y="3.75" width="1.5" height="3.75" fill="currentColor" />
		<rect
			className={classes(styles["robot-eye"], styles["robot-eye-left"])}
			x="3.75"
			y="3.5"
			width="1.5"
			height="2"
			fill="currentColor"
		/>
		<rect
			className={classes(styles["robot-eye"], styles["robot-eye-right"])}
			x="6.75"
			y="3.5"
			width="1.5"
			height="2"
			fill="currentColor"
		/>
		<path
			className={styles["robot-smile"]}
			d="M3.375 7C4.987 7.708 6.868 7.772 8.625 7"
			pathLength="1"
			stroke="currentColor"
			strokeWidth="1.5"
		/>
	</svg>
);

/** Where the thing a {@link MetaStatus} names has got to, which sets its glyph and its colour. */
export type MetaStatusVariant =
	/** Amber spinner: under way, as checks that are running. */
	| "working"
	/** Green tick: done and fine, as checks that passed. */
	| "passed"
	/** Red cross: done and broken, as a check that failed. */
	| "failed"
	/** Amber warning: stopped until you act, as a check that needs approval. */
	| "warning"
	/** Grey dashed ring: no result yet, as checks that are pending, queued, cancelled or unknown. */
	| "neutral";

const statusGlyph: Record<MetaStatusVariant, IconName> = {
	working: "spinner",
	passed: "tick",
	failed: "cross",
	warning: "warning",
	neutral: "pending",
};

const statusWord: Record<MetaStatusVariant, string> = {
	working: "running",
	passed: "passed",
	failed: "failed",
	warning: "needs you",
	neutral: "no result",
};

/**
 * Where something that runs has got to, as a glyph and a short label in its state's colour: a
 * pull request's checks as "CI", a deploy, a sync. It sits in a row's meta beside the
 * {@link MetaCount}s, or after a tab's label.
 *
 * The label names the thing, not its state, which the glyph and colour say: "CI", not
 * "CI passed". Where there is room, as a pull request's tab, it can say how far it got,
 * "CI 2/14". A screen reader hears the label and the state, "CI, running".
 *
 * Without a label it is the glyph alone, where what it is about is already plain, as beside a
 * pull request's title. Give it an `aria-label` then that names both, "Checks running".
 *
 * @figma `label` turns the label off; `label text` is its words.
 *
 * @see packages/but-ui/design/patterns/work-under-way.md
 * @import import { MetaStatus } from "@gitbutler/but-ui-react/MetaCount.tsx";
 */
export const MetaStatus: FC<
	{ variant: MetaStatusVariant } & Omit<ComponentProps<"span">, "children"> &
		({ children: string } | { children?: undefined; "aria-label": string })
> = ({ variant, children, ...props }) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph and a word with one name, not a picture with a source.
		role="img"
		aria-label={children === undefined ? undefined : `${children}, ${statusWord[variant]}`}
		{...props}
		className={classes(props.className, "text-13", styles.count, styles[variant])}
		// A mark with no edge of its own, which a row sets in further when it ends on one.
		data-meta-count=""
	>
		<Icon name={statusGlyph[variant]} size={14} className={classes(styles.glyph, styles.faded)} />
		{children !== undefined && <span className={styles.value}>{children}</span>}
	</span>
);
