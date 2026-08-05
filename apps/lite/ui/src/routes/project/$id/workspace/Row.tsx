import { classes } from "#ui/components/classes.ts";
import { Checkbox } from "#ui/components/Checkbox.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	type ReactNode,
	useLayoutEffect,
	useRef,
} from "react";
import styles from "./Row.module.css";
import { mergeProps, useRender } from "@base-ui/react";

const isFromInteractiveDescendant = (event: MouseEvent<HTMLDivElement>): boolean => {
	if (!(event.target instanceof Element)) return false;
	const interactiveElement = event.target.closest(["button", "input[type='checkbox']"].join(","));
	return interactiveElement !== null && event.currentTarget.contains(interactiveElement);
};

export const Row: FC<
	{
		isSelected?: boolean;
		onSelect?: () => void;
		/** @default false */
		isHighlighted?: boolean;
		/**
		 * Whether the row's checkbox is checked. The row needs this on the
		 * container — rather than reading it off the checkbox with `:has()` — so
		 * that a row can also style itself against a *neighbouring* row's checked
		 * state, which would otherwise require nesting `:has()` inside `:has()`.
		 */
		isChecked?: boolean;
		interactive?: boolean;
	} & Omit<ComponentProps<"div">, "onSelect">
> = ({
	isSelected,
	onSelect,
	isHighlighted,
	isChecked,
	interactive = true,
	ref: refProp,
	...props
}) => {
	const rowRef = useRef<HTMLDivElement | null>(null);
	const mergedRef = useMergedRefs(rowRef, refProp);

	useLayoutEffect(() => {
		if (!isSelected) return;
		rowRef.current?.scrollIntoView({
			block: "nearest",
			inline: "nearest",
		});
	}, [isSelected]);

	return (
		// This is safe because the tree is focusable.
		// oxlint-disable-next-line jsx_a11y/click-events-have-key-events, jsx_a11y/no-static-element-interactions
		<div
			{...props}
			ref={mergedRef}
			className={classes(
				props.className,
				styles.container,
				isChecked && styles.containerChecked,
				isSelected && styles.containerSelected,
				isHighlighted && styles.containerHighlighted,
				interactive && styles.containerInteractive,
			)}
			onMouseDown={(event) => {
				props.onMouseDown?.(event);

				if (
					!event.defaultPrevented &&
					// Prevent clicks on interactive descendants from stealing focus from the tree.
					isFromInteractiveDescendant(event)
				)
					event.preventDefault();
			}}
			onClick={(event) => {
				props.onClick?.(event);

				if (
					!event.defaultPrevented &&
					// Prevent clicks on interactive descendants from stealing selection.
					!isFromInteractiveDescendant(event)
				)
					onSelect?.();
			}}
		/>
	);
};

export const RowCheckbox: FC<ComponentProps<typeof Checkbox>> = (props) => (
	<Checkbox
		{...props}
		className={(state) =>
			classes(
				styles.rowCheckbox,
				typeof props.className === "function" ? props.className(state) : props.className,
			)
		}
	/>
);

/**
 * The fold control on a row's graph rail: `glyph` at rest, a chevron once the
 * row is hovered or focused. Both are always rendered and the swap is CSS-only
 * (see `.foldToggle`), which blanks just the glyph's own segment so the rail
 * below it keeps drawing. The chevron direction reports the state the way a
 * disclosure triangle does.
 *
 * `foldedIndicator` marks the rail for as long as the row is folded. The
 * chevron only appears on hover, so it cannot carry that on its own, and
 * `glyph` is busy describing the row's position in the graph. It takes the
 * second line, leaving the first to the glyph and the chevron.
 */
export const RowFoldToggle: FC<
	{ folded: boolean; glyph: ReactNode; foldedIndicator?: ReactNode } & ComponentProps<"button">
> = ({ folded, glyph, foldedIndicator, ...props }) => (
	<button
		type="button"
		{...props}
		aria-expanded={!folded}
		className={classes(props.className, styles.foldToggle)}
	>
		<span className={styles.foldGlyph}>{glyph}</span>

		{folded && foldedIndicator !== undefined && (
			<span className={styles.foldIndicator}>{foldedIndicator}</span>
		)}

		<span className={styles.foldChevron}>
			<Icon size={14} name={folded ? "chevron-right" : "chevron-down"} />
		</span>
	</button>
);

export const RowLabelContainer: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.labelContainer)} />
);

export const RowLabelFooter: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.labelFooter)} />
);

/** The label column of a two-line row: heading above, {@link RowMeta} below. */
export const RowLabelGroup: FC<ComponentProps<"div">> = (props) => (
	<div {...props} className={classes(props.className, styles.labelGroup)} />
);

/**
 * A row's second line: small, muted facts about its subject. Items go in with
 * `Row.module.css`'s `metaItem`, which any element can take — several are
 * links.
 */
export const RowMeta: FC<ComponentProps<"div">> = (props) => (
	<RowLabelFooter {...props} className={classes(props.className, "text-13", styles.meta)} />
);

export const RowLabel: FC<
	{ heading?: boolean; singleLine?: boolean } & useRender.ComponentProps<"div">
> = ({ heading, singleLine, render, ...props }) =>
	useRender({
		render,
		props: mergeProps<"div">(props, {
			className: classes(
				styles.label,
				singleLine && styles.labelSingleLine,
				heading ? "text-14" : "text-13",
				heading && "text-bold",
			),
		}),
	});

/**
 * A non-interactive row that acts as a section header (e.g. "Stacks", "Uncommitted changes").
 *
 * `children` are rendered inside the label container next to the heading (e.g. badges),
 * while `actions` are rendered outside of it (e.g. a toolbar).
 */
export const SectionHeaderRow: FC<
	{ label: ReactNode; actions?: ReactNode } & Omit<
		ComponentProps<typeof Row>,
		"interactive" | "onSelect" | "isSelected"
	>
> = ({ label, actions, children, ...props }) => (
	<Row {...props} className={classes(props.className, styles.sectionHeader)} interactive={false}>
		<RowLabelContainer>
			<RowLabel heading>{label}</RowLabel>

			{children}
		</RowLabelContainer>

		{actions}
	</Row>
);

/** @public */
export const RowBubbleGroup: FC<ComponentProps<"span">> = (props) => (
	<span {...props} className={classes(props.className, styles.bubbleGroup)} />
);

export const RowToolbar: FC<{ forceVisible?: boolean } & ComponentProps<"div">> = ({
	forceVisible,
	...props
}) => (
	<div
		{...props}
		className={classes(props.className, styles.toolbar, forceVisible && styles.toolbarForceVisible)}
	/>
);
