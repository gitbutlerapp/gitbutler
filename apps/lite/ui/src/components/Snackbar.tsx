import { classes } from "#ui/components/classes.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import styles from "./Snackbar.module.css";
import { Button } from "@base-ui/react";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";

/** @public */
export type SnackbarVariant =
	/** Something happened worth saying, with no verdict attached. */
	| "info"
	/** An act that failed, or refused to run. */
	| "danger"
	/** An act that came off. */
	| "safe";

/** The glyph a variant leads with when the caller names none of its own. */
const defaultIcon = (variant: SnackbarVariant): IconName =>
	Match.value(variant).pipe(
		Match.when("info", () => "info" as const),
		Match.when("danger", () => "danger" as const),
		Match.when("safe", () => "tick" as const),
		Match.exhaustive,
	);

/**
 * One line of consequence, floated over the surface that caused it: a glyph, a sentence, and — when
 * it needs dismissing by hand — a way out fenced off behind a divider. A whole {@link Toasts} card
 * would state a title and a description; a snackbar has only the sentence, so it can sit close to
 * the thing it is about rather than in the corner of the window.
 *
 * Every variant wears the same surface: the verdict is carried by the glyph alone, so a run of
 * snackbars reads as one row of statements rather than a traffic light.
 *
 * @public
 */
export const Snackbar: FC<
	{
		variant?: SnackbarVariant;
		/** Overrides the variant's own glyph. */
		icon?: IconName;
		/** Given, the snackbar carries a close button; otherwise it says its piece and nothing more. */
		onDismiss?: () => void;
		dismissLabel?: string;
	} & ComponentProps<"div">
> = ({ variant = "info", icon, onDismiss, dismissLabel = "Dismiss", children, ...props }) => (
	<div
		// A failure interrupts; the other two are there to be read whenever the reader gets to them.
		role={variant === "danger" ? "alert" : "status"}
		{...props}
		className={classes(
			props.className,
			styles.snackbar,
			"text-12",
			Match.value(variant).pipe(
				Match.when("info", () => styles.info),
				Match.when("danger", () => styles.danger),
				Match.when("safe", () => styles.safe),
				Match.exhaustive,
			),
		)}
	>
		<Icon name={icon ?? defaultIcon(variant)} size={14} className={styles.icon} />
		<span className={styles.message}>{children}</span>
		{onDismiss !== undefined && (
			<>
				<div aria-hidden className={styles.divider} />
				<Button
					aria-label={dismissLabel}
					className={classes(
						getButtonClassName({ variant: "ghost", size: "small", iconOnly: true }),
						styles.dismiss,
					)}
					onClick={onDismiss}
				>
					<Icon name="cross" />
				</Button>
			</>
		)}
	</div>
);
