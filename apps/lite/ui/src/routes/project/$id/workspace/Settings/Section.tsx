import { classes } from "#ui/components/classes.ts";
import styles from "./Section.module.css";
import type { FC, ReactNode } from "react";

type SectionProps = {
	/** Omit on a page's first section, where the page's own label already says it. */
	heading?: string;
	children: ReactNode;
};

/** A group of rows whose labels and controls share a column pair. */
export const Section: FC<SectionProps> = (p) => (
	<section className={styles.section}>
		{p.heading !== undefined && (
			<h2 className={classes("text-13", "text-semibold", styles.heading)}>{p.heading}</h2>
		)}
		<div className={classes("text-13", styles.rows)}>{p.children}</div>
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
	children: ReactNode;
};

/**
 * One setting. A control that a `<label>` can own takes `htmlFor`; anything else — a
 * toggle group, a set of buttons — takes `labelId` and points at it, which is why the
 * label is not always a `<label>`.
 */
export const Row: FC<RowProps> = (p) => (
	<div className={classes(styles.row, p.stacked && styles.stacked)}>
		{p.htmlFor === undefined ? (
			<span id={p.labelId} className={classes("text-semibold", styles.label)}>
				{p.label}
			</span>
		) : (
			<label htmlFor={p.htmlFor} className={classes("text-semibold", styles.label)}>
				{p.label}
			</label>
		)}

		<div className={styles.control}>{p.children}</div>

		{/*
		 * On its own line under both columns. Kept in the label cell it would be the
		 * widest thing there, and the label column sizes to its content — a sentence
		 * of explanation would take the width the control needs.
		 */}
		{p.hint !== undefined && <span className={classes("text-12", styles.hint)}>{p.hint}</span>}
	</div>
);
