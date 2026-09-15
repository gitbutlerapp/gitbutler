import { classes } from "#ui/components/classes.ts";
import styles from "./Section.module.css";
import type { FC, ReactNode } from "react";

type SectionProps = {
	/** Omit on a page's first section, where the page's own label already says it. */
	heading?: string;
	/**
	 * A recessed strip closing the card, for what the rows add up to rather than another setting:
	 * a connection's status and the button that checks it.
	 */
	footer?: ReactNode;
	children: ReactNode;
};

/** A card of rows, one setting each. */
export const Section: FC<SectionProps> = (p) => (
	<section className={styles.section}>
		{p.heading !== undefined && (
			<h2 className={classes("text-12", "text-semibold", styles.heading)}>{p.heading}</h2>
		)}
		<div className={styles.rows}>
			{p.children}
			{p.footer !== undefined && <div className={styles.footer}>{p.footer}</div>}
		</div>
	</section>
);

type RowProps = {
	label: string;
	/** Places the control below the label and hint, spanning the row. */
	stacked?: boolean;
	/** Ties the label to a native control. Composite widgets pass `labelId` instead. */
	htmlFor?: string;
	/** Names the label so a composite widget can point `aria-labelledby` at it. */
	labelId?: string;
	/** Sits under the label: a unit, an inferred value, a caveat. */
	hint?: ReactNode;
	/**
	 * Sits under the hint, flush with the words rather than at the row's end: a button that does
	 * once what the control at the end does on its own.
	 */
	below?: ReactNode;
	children?: ReactNode;
};

/**
 * One setting: its name and, under that, a line saying what it does, with the control at the
 * row's end. A control that a `<label>` can own takes `htmlFor`; anything else — a toggle group,
 * a set of buttons — takes `labelId` and points at it, which is why the label is not always a
 * `<label>`.
 */
export const Row: FC<RowProps> = (p) => (
	<div
		className={classes(
			styles.row,
			p.stacked && styles.stacked,
			// A row that is one line of text centres its control on that line; one with a hint
			// keeps the control up against the label, which is what it belongs to.
			p.hint === undefined && styles.centered,
		)}
	>
		<div className={styles.text}>
			{p.htmlFor === undefined ? (
				<span id={p.labelId} className={classes("text-15", "text-semibold", styles.label)}>
					{p.label}
				</span>
			) : (
				<label htmlFor={p.htmlFor} className={classes("text-15", "text-semibold", styles.label)}>
					{p.label}
				</label>
			)}

			{p.hint !== undefined && (
				<span className={classes("text-12", "text-body", styles.hint)}>{p.hint}</span>
			)}

			{p.below !== undefined && <div className={styles.below}>{p.below}</div>}
		</div>

		<div className={styles.control}>{p.children}</div>
	</div>
);
