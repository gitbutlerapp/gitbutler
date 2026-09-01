import { classes } from "#ui/components/classes.ts";
import { Icon } from "./Icon.tsx";
import type { IconName } from "./iconNames.ts";
import { Popup } from "#ui/components/Popup.tsx";
import styles from "./Toolbox.module.css";
import type { ComponentProps, FC, ReactNode } from "react";

/**
 * Toolboxes that belong to one operation, stacked. An addon — a type selector qualifying what the
 * toolbox below it will do — is a card of its own rather than a section, so it reads as a question
 * asked before the operation rather than a step within it.
 *
 * @public
 */
export const ToolboxStack: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.stack)} />
);

/**
 * A floating card of things to do with what is currently in hand — a checked set, a transfer
 * being aimed, an absorb waiting on its plan. It states its subject in a {@link ToolboxMeta}
 * strip and stacks one {@link ToolboxSection} per decision the subject still needs.
 *
 * @public
 */
export const Toolbox: FC<ComponentProps<"div">> = (props) => (
	<Popup {...props} className={classes(props.className, styles.toolbox)} />
);

/**
 * What the sections below are about, as a line of muted text. `strong` children read as the
 * subject's own words against it.
 *
 * @public
 */
export const ToolboxMeta: FC<ComponentProps<"div"> & { icon?: IconName }> = ({
	icon,
	children,
	...props
}) => (
	<div {...props} className={classes(props.className, styles.meta, "text-12")}>
		{icon !== undefined && <Icon name={icon} size={14} className={styles.metaIcon} />}
		{children}
	</div>
);

/**
 * A run of {@link ToolboxMeta} that gives up its width before the words around it do, so a long
 * commit message ellipsises rather than pushing the rest of the sentence out of the card.
 *
 * @public
 */
export const ToolboxMetaText: FC<{ children: ReactNode }> = ({ children }) => (
	<span className={styles.metaText}>{children}</span>
);

/**
 * A hint pinned to the end of the strip. The toolbox's own acts wear their chords, so the one that
 * closes it says so here rather than taking a labelled button's worth of the row.
 *
 * @public
 */
export const ToolboxMetaHint: FC<{ children: ReactNode }> = ({ children }) => (
	<span className={styles.metaHint}>{children}</span>
);

/** @public */
export type ToolboxSectionVariant =
	/** Acts, packed from the start. */
	| "actions"
	/** One control — a toggle group — filling the width. */
	| "stretch"
	/** What ends the operation, gathered at the end. */
	| "confirm";

/** @public */
export const ToolboxSection: FC<ComponentProps<"div"> & { variant?: ToolboxSectionVariant }> = ({
	variant = "actions",
	...props
}) => (
	<div
		{...props}
		className={classes(
			props.className,
			styles.section,
			variant === "stretch" && styles.stretch,
			variant === "confirm" && styles.confirm,
		)}
	/>
);

/**
 * Divides one run of acts from another within a section — what leaves the operation from what
 * carries it out.
 *
 * @public
 */
export const ToolboxSeparator: FC = () => <div className={styles.separator} />;
