import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { ComponentProps, FC, ReactNode } from "react";
import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { Kbd } from "./Kbd.tsx";
import styles from "./StatusBar.module.css";

type StatusBarProps = {
	/** What needs attention: notes, a {@link StatusAlert}, a {@link StatusLocation}, a {@link StatusText}; or, while something is under way, one {@link StatusOperation}. */
	start?: ReactNode;
	/** What you can do: {@link StatusHint}s for the selection, then a note that opens the commands, then at most one small outline `Button` the window offers on its own, as "Build and restart" in a dev build. */
	end?: ReactNode;
} & Omit<ComponentProps<"footer">, "children">;

/**
 * The window's footer, as 🍑 But UI's StatusBar draws it: what needs attention at the start, what
 * you can do at the end. The start holds "Fetched 2m ago", then a {@link StatusAlert} with the
 * conflicts, then the other notes (1 unread, CI failed), then a {@link StatusLocation} or a
 * {@link StatusText}. While a drag, a cut or an operation is under way it holds a
 * {@link StatusOperation} instead. The end holds the keys, then ⌘K Commands, then, only when the
 * window has one of its own, a small outline `Button` (Build and restart in a dev build).
 *
 * A note that doesn't fit drops whole from the end of the start, so put them in the order they
 * matter. An operation doesn't drop: its subject and place are ellipsed instead.
 *
 * A failed fetch replaces "Fetched" with a danger {@link StatusNote}, "Couldn't fetch".
 *
 * @figma The GPUI client draws its own, in `crates/but-gpui/src/status_bar.rs`.
 * @import import { StatusBar } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusBar: FC<StatusBarProps> = ({ start, end, ...props }) => (
	<footer {...props} className={classes(props.className, styles.bar)}>
		<div className={styles.start}>{start}</div>
		{end !== undefined && <div className={styles.end}>{end}</div>}
	</footer>
);

type StatusNoteProps = {
	/** `danger` for something broken (CI failed, Couldn't fetch) and for the counts in a {@link StatusAlert}. @default "default" */
	tone?: "default" | "danger";
	/** The glyph before the label. */
	icon?: IconName;
	/** The glyph after the label, as the refresh arrow on "Fetched 2m ago". */
	iconEnd?: IconName;
} & ComponentProps<"button">;

/**
 * A footer button: quiet text/2 at rest, so the bar stays calm until something needs attention.
 * The default tone is for status and the ways in (Fetched 2m ago, 1 unread, ⌘K Commands);
 * `danger` puts text/danger on a danger tint. Its label states a count and what it is, never a verb.
 *
 * @import import { StatusNote } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusNote: FC<StatusNoteProps> = ({
	tone = "default",
	icon,
	iconEnd,
	children,
	...props
}) => (
	<button
		type="button"
		{...props}
		className={classes(props.className, "text-12", styles.note, tone === "danger" && styles.danger)}
	>
		{icon !== undefined && <Icon name={icon} size={14} />}
		{children}
		{iconEnd !== undefined && <Icon name={iconEnd} size={14} />}
	</button>
);

/**
 * What needs resolving, behind one danger badge so its counts read as one problem: the conflicts.
 * Its children are danger {@link StatusNote}s, one per place, as "3 commits" and "2 uncommitted
 * files". Leave out a note whose count is 0, and the whole alert when both are.
 *
 * @import import { StatusAlert } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusAlert: FC<ComponentProps<"span">> = ({ children, ...props }) => (
	<span {...props} className={classes(props.className, styles.alert)}>
		{/* oxlint-disable-next-line jsx_a11y/prefer-tag-over-role -- A glyph with a name, not a picture with a source. */}
		<span role="img" aria-label="Conflicts" className={styles.badge}>
			<Icon name="warning" size={14} />
		</span>
		{children}
	</span>
);

type StatusLocationProps = {
	/** @default "Editing in" */
	verb?: ReactNode;
	/** The worktree's name, as "Main worktree" or "feature-x", never a path. */
	place: ReactNode;
} & Omit<ComponentProps<"span">, "children">;

/**
 * Where edits land: the worktree the workspace writes to, last in the start after the notes.
 *
 * @import import { StatusLocation } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusLocation: FC<StatusLocationProps> = ({
	verb = "Editing in",
	place,
	...props
}) => (
	<span {...props} className={classes(props.className, "text-12", styles.location)}>
		<Icon name="edit" size={14} />
		{verb}
		<Icon name="folder" size={14} />
		{place}
	</span>
);

/**
 * Plain words in the start of a {@link StatusBar}, quiet text/2 that isn't a way in: where you are
 * as you step through something, as "File 12 of 36" while going through a diff's files.
 *
 * @import import { StatusText } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusText: FC<ComponentProps<"span">> = (props) => (
	<span {...props} className={classes(props.className, "text-12", styles.text)} />
);

type StatusHintProps = {
	// Not `Hotkey`, for the reason Kbd gives.
	hotkey: string | HotkeySequence;
} & ComponentProps<"span">;

/**
 * A key and what it does, in the end of a {@link StatusBar}. It isn't clickable: the key is the
 * way in. The first hint is the state's own keys (↵ Drop here, ⎋ Cancel while moving), then the
 * selection's; undo names the last action, as "⌘Z Undo move commit".
 *
 * @import import { StatusHint } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusHint: FC<StatusHintProps> = ({ hotkey, children, ...props }) => (
	<span {...props} className={classes(props.className, "text-13", styles.hint)}>
		<Kbd hotkey={hotkey} />
		{children}
	</span>
);

/** What a {@link StatusOperation} does to what it takes. */
export type StatusOperationKind = "move" | "squash" | "amend" | "commit" | "cherry-pick" | "cut";

const operations: Record<StatusOperationKind, { icon: IconName; verb: string }> = {
	move: { icon: "arrow-right", verb: "Moving" },
	squash: { icon: "absorb", verb: "Squashing" },
	amend: { icon: "absorb", verb: "Amending" },
	commit: { icon: "plus", verb: "Committing" },
	"cherry-pick": { icon: "copy", verb: "Cherry-picking" },
	cut: { icon: "cut", verb: "Cut" },
};

type StatusOperationProps = {
	/** @default "move" */
	op?: StatusOperationKind;
	/** What the operation takes: one commit by its title, several as "3 commits", a branch, file or worktree by its name. Commit and amend have none, as the composer shows what they take. */
	subject?: ReactNode;
	/** The glyph before the subject, as `commit` or `branch`. */
	subjectIcon?: IconName;
	/** Where it lands, relative to the place: "to the top of", "below", "above", "into", "to a new branch below", "to a new stack". */
	where?: ReactNode;
	/** What it lands on or by, as a commit's title or a branch's name. */
	place?: ReactNode;
	/** The glyph before the place. */
	placeIcon?: IconName;
	/** The operation has been asked for and is under way: a clock, and every word in text/2. */
	running?: boolean;
	/** How long a running operation has taken, once that's noticeable, as "3s". */
	elapsed?: ReactNode;
} & Omit<ComponentProps<"span">, "children">;

/**
 * What's under way, in the start of a {@link StatusBar} in place of its notes: what a drag, a cut
 * or a running operation does. The operation's glyph and verb come first in text/pop, then what it
 * takes and where it lands in text/2, as "→ Moving · ⟜ Add test file 18 · to the top of ·
 * ⑂ pl-branch-3". Once asked for it's `running`: a clock in place of the glyph, the verb in text/2
 * too, and the time taken in text/3 once it's noticeable.
 *
 * The subject and the place give way first when the bar runs out of room, each ellipsed on its own.
 *
 * @import import { StatusOperation } from "@gitbutler/but-ui-react/StatusBar.tsx";
 */
export const StatusOperation: FC<StatusOperationProps> = ({
	op = "move",
	subject,
	subjectIcon,
	where,
	place,
	placeIcon,
	running = false,
	elapsed,
	...props
}) => {
	const { icon, verb } = operations[op];
	return (
		<span
			{...props}
			className={classes(props.className, "text-12", styles.operation, running && styles.running)}
		>
			<Icon name={running ? "clock" : icon} size={14} className={styles.op} />
			<span className={styles.op}>{verb}</span>
			{subjectIcon !== undefined && <Icon name={subjectIcon} size={14} />}
			{subject !== undefined && <span className={styles.name}>{subject}</span>}
			{where !== undefined && <span>{where}</span>}
			{placeIcon !== undefined && <Icon name={placeIcon} size={14} />}
			{place !== undefined && <span className={styles.name}>{place}</span>}
			{running && elapsed !== undefined && <span className={styles.elapsed}>{elapsed}</span>}
		</span>
	);
};
