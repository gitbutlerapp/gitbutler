import { classes } from "./classes.ts";
import { Button } from "./Button.tsx";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./Snackbar.module.css";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";

/** @public */
export type SnackbarVariant =
	/** Something happened worth saying, with no verdict attached. */
	| "info"
	/** An act that came off with a catch: it ran, but not quite as asked. */
	| "warning"
	/** An act that failed, or refused to run. */
	| "danger"
	/** An act that came off. */
	| "safe";

/** The glyph a variant leads with when the caller names none of its own. */
const defaultIcon = (variant: SnackbarVariant): IconName =>
	Match.value(variant).pipe(
		Match.when("info", () => "info" as const),
		Match.when("warning", () => "warning" as const),
		Match.when("danger", () => "danger" as const),
		Match.when("safe", () => "tick" as const),
		Match.exhaustive,
	);

/**
 * One line of consequence, floated over the surface that caused it: a glyph, a sentence, and at most
 * one button, fenced off behind a divider: Undo when what just happened can be taken back, or a close
 * button when the snackbar waits to be dismissed by hand. Never both: an undoable snackbar leaves on
 * its own, so it has nothing to close. A whole {@link Toasts} card
 * would state a title and a description; a snackbar has only the sentence, so it can sit close to
 * the thing it is about rather than in the corner of the window.
 *
 * Every variant wears the same surface: the verdict is carried by the glyph alone, so a run of
 * snackbars reads as one row of statements rather than a traffic light.
 *
 * @public
 * @import import { Snackbar } from "@gitbutler/but-ui-react/Snackbar.tsx";
 */
export const Snackbar: FC<
	{
		variant?: SnackbarVariant;
		/** Overrides the variant's own glyph. */
		icon?: IconName;
		dismissLabel?: string;
		undoLabel?: string;
	} & (
		| {
				/** Given, the snackbar carries a close button; otherwise it says its piece and nothing more. */
				onDismiss?: () => void;
				onUndo?: never;
		  }
		| {
				/** Given, the snackbar carries Undo, which takes back what it says happened. */
				onUndo: () => void;
				onDismiss?: never;
		  }
	) &
		ComponentProps<"div">
> = ({
	variant = "info",
	icon,
	onDismiss,
	dismissLabel = "Dismiss",
	onUndo,
	undoLabel = "Undo",
	children,
	...props
}) => (
	<div
		// A failure interrupts; the rest are there to be read whenever the reader gets to them.
		role={variant === "danger" ? "alert" : "status"}
		{...props}
		className={classes(
			props.className,
			styles.snackbar,
			"text-12",
			Match.value(variant).pipe(
				Match.when("info", () => styles.info),
				Match.when("warning", () => styles.warning),
				Match.when("danger", () => styles.danger),
				Match.when("safe", () => styles.safe),
				Match.exhaustive,
			),
		)}
	>
		<Icon name={icon ?? defaultIcon(variant)} size={14} className={styles.icon} />
		<span className={styles.message}>{children}</span>
		{(onUndo !== undefined || onDismiss !== undefined) && (
			<div aria-hidden className={styles.divider} />
		)}
		{onUndo !== undefined ? (
			<Button variant="ghost-inverted" size="small" className={styles.action} onClick={onUndo}>
				<Icon name="undo" />
				{undoLabel}
			</Button>
		) : (
			onDismiss !== undefined && (
				<Button
					aria-label={dismissLabel}
					variant="ghost-inverted"
					size="small"
					iconOnly
					className={styles.action}
					onClick={onDismiss}
				>
					<Icon name="cross" />
				</Button>
			)
		)}
	</div>
);
