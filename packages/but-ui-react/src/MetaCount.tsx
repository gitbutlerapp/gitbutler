import type { ComponentProps, FC, MouseEventHandler } from "react";
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
	head: "branch",
	uncommitted: "diff",
	clean: "diff",
	age: "clock",
	branch: "branch",
	worktrees: "folder",
	repos: "repo",
	"agent-working": "robot",
	"agent-waiting": "robot",
	stack: "stack",
	cpu: "cpu",
	"cpu-danger": "cpu-danger",
};

type MetaCountProps = { type: MetaCountType } & (
	| ({ onClick?: undefined } & Omit<ComponentProps<"span">, "onClick">)
	// A button's own `type` (submit, reset) would clash with what the count counts.
	| ({ onClick: MouseEventHandler<HTMLButtonElement> } & Omit<
			ComponentProps<"button">,
			"onClick" | "type"
	  >)
);

/**
 * A glyph and a short value in a row's meta: a count, an age, a branch's name. The type sets both.
 * Amber is work waiting on you (`behind`, `uncommitted`, `agent-waiting`), blue is work to send
 * (`unpushed`), green is an agent at work (`agent-working`); the rest are neutral, `stack` among
 * them: where a branch sits in its stack, "1/3 in stack". `rebased` is the unpushed arrow in that
 * neutral grey: commits that were pushed and have been rewritten since, most often by a rebase onto
 * the target, so a push replaces them rather than sending new work. Red is what can't go on until
 * you step in: `conflicts`, how many of a branch's commits are conflicted. Quietest of all
 * are `empty`, a branch with no commits yet, and `head`, what a worktree with nothing to list is on
 * (`empty`, `detached`); a row shows either first, ahead of its counts. `cpu` is a process's CPU
 * use while it stays low, in green; `cpu-danger` is the same use once it runs high, in red with a
 * warning on the chip.
 *
 * The value is the children, so a host formats it (`12`, `3d`, `retry-transfers`). The glyph says
 * nothing to a screen reader: wrap the count in a `Tooltip` saying what it counts, or give it an
 * `aria-label`. A long value, such as a branch's name, ends in an ellipsis once its row runs out
 * of room.
 *
 * With `onClick` the count is the button for its next step: ↓ `behind` updates, ↑ `unpushed`
 * pushes. It looks the same at rest and takes up the same room; under the pointer it sits on its
 * own colour at 14% (a grey one on the hover ground) and keeps its text, and keyboard focus shows
 * the ring.
 * Its `Tooltip` then says what pressing it does ("Push 4 commits"), not what it counts. The row
 * under it still gets the click, so a host that selects rows stops it there.
 *
 * @import import { MetaCount } from "@gitbutler/but-ui-react/MetaCount.tsx";
 */
export const MetaCount: FC<MetaCountProps> = (props) => {
	const { type, children } = props;
	const content = (
		<>
			{type === "agent-working" ? (
				<WorkingRobot />
			) : (
				<Icon name={glyph[type]} size={14} className={styles.glyph} />
			)}
			<span className={styles.value}>{children}</span>
		</>
	);
	const className = classes(props.className, "text-13", styles.count, styles[type]);
	if (props.onClick === undefined) {
		const { type: _type, children: _children, ...rest } = props;
		return (
			<span {...rest} className={className}>
				{content}
			</span>
		);
	}
	const { type: _type, children: _children, ...rest } = props;
	return (
		<button type="button" {...rest} className={classes(className, styles.pressable)}>
			{content}
		</button>
	);
};

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
		className={styles.glyph}
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
 * Its words are "CI" unless given others: where there is room, as a pull request's tab, how far
 * the checks got, "CI 2/14".
 *
 * @import import { CiStatus } from "@gitbutler/but-ui-react/MetaCount.tsx";
 */
export const CiStatus: FC<{ status: CiStatusType } & ComponentProps<"span">> = ({
	status,
	children = "CI",
	...props
}) => (
	<span
		// oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph and a word with one name, not a picture with a source.
		role="img"
		aria-label={
			typeof children === "string" && children !== "CI"
				? `${ciLabel[status]}, ${children}`
				: ciLabel[status]
		}
		{...props}
		className={classes(props.className, "text-13", styles.count, styles[status])}
	>
		<Icon name={ciGlyph[status]} size={14} className={styles.glyph} />
		<span className={styles.value}>{children}</span>
	</span>
);
