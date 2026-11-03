import { classes } from "./classes.ts";
import { Icon } from "./Icon.tsx";
import styles from "./Steps.module.css";
import { Match } from "effect";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * A short sequence the user follows in order, one `Step` each: sign in, then run. Each is
 * numbered and joined to the next by a line. For items that don't follow on from each other, use
 * `List`.
 *
 * @public
 * @import import { Steps, Step } from "@gitbutler/ui-react/Steps.tsx";
 */
export const Steps: FC<ComponentProps<"ol">> = ({ className, ...props }) => (
	<ol {...props} className={classes(styles.steps, className)} />
);

/** @public */
export type StepVariant = "lightGray" | "fillGray" | "safe";

/**
 * One step of {@link Steps}: its number, a title, and what the step needs under it, a
 * `CodeBlock`, a `DescriptionList` or a hint.
 *
 * `variant` takes `Badge`'s tones: `lightGray` for steps to follow in any order, `fillGray` for the
 * step the user is on, `safe` for one that passed. `done` shows a tick instead of the number.
 *
 * @public
 * @import import { Steps, Step } from "@gitbutler/ui-react/Steps.tsx";
 */
export const Step: FC<{
	title: ReactNode;
	description?: ReactNode;
	variant?: StepVariant;
	done?: boolean;
	children?: ReactNode;
}> = ({ title, description, variant = "lightGray", done = false, children }) => (
	<li className={styles.step}>
		<span className={styles.rail}>
			{/* The list numbers the steps for a screen reader already; only a tick needs saying. */}
			<span
				{...(done ? { role: "img", "aria-label": "Done" } : { "aria-hidden": true })}
				className={classes(
					"text-11",
					"text-semibold",
					styles.marker,
					!done && styles.number,
					Match.value(variant).pipe(
						Match.when("lightGray", () => styles.lightGray),
						Match.when("fillGray", () => styles.fillGray),
						Match.when("safe", () => styles.safe),
						Match.exhaustive,
					),
				)}
			>
				{done && <Icon name="tick" />}
			</span>
			<span className={styles.connector} />
		</span>
		<div className={styles.body}>
			<div className={styles.header}>
				<div className={classes("text-13", "text-semibold", styles.title)}>{title}</div>
				{description !== undefined && (
					<div className={classes("text-12", "text-body", styles.description)}>{description}</div>
				)}
			</div>
			{children}
		</div>
	</li>
);
