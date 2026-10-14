import { Button, getButtonClassName } from "./Button.tsx";
import { ChangeScale } from "./ChangeScale.tsx";
import { classes } from "./classes.ts";
import { DiffStats } from "./DiffStats.tsx";
import { FileIcon } from "./FileIcon.tsx";
import { Icon } from "./Icon.tsx";
import { describeLineStats } from "./lineStats.ts";
import { Tooltip } from "./Tooltip.tsx";
import { Toolbar } from "@base-ui/react";
import type { HotkeySequence } from "@tanstack/react-hotkeys";
import type { ComponentProps, FC, MouseEvent, ReactNode } from "react";
import styles from "./DiffFileHeader.module.css";

/**
 * Where the reader stands with a file: reviewed, reviewed but changed since,
 * or not reviewed yet.
 */
export type DiffFileReviewState = "reviewed" | "changed" | "unreviewed";

type Props = {
	/** The file's path. The header leads with its name and dims the folder after it. */
	path: string;
	/** The file's changed line counts. Leave both out when there is no patch to count. */
	added?: number;
	removed?: number;
	collapsed?: boolean;
	/** Shows the fold button. Without it the header has none. */
	onCollapsedChange?: (collapsed: boolean) => void;
	/** The shortcut that folds the file, shown in the fold button's tooltip. */
	collapseKbd?: string | HotkeySequence;
	/** The focus scope that shortcut acts in; see `Tooltip`'s `kbdScope`. */
	collapseKbdScope?: string;
	reviewState?: DiffFileReviewState;
	/** Shows the Reviewed button. Without it the header has none. */
	onReviewedChange?: (reviewed: boolean) => void;
	/** Shows the kebab button; opens the host's file menu from it. */
	onMenu?: (event: MouseEvent<HTMLButtonElement>) => void;
	/** Anything the file needs said before its actions, such as a `Badge`. */
	children?: ReactNode;
} & Omit<ComponentProps<"header">, "children">;

const reviewLabels: Record<DiffFileReviewState, string> = {
	reviewed: "Reviewed",
	changed: "Needs review",
	unreviewed: "Not reviewed",
};

/**
 * The header over one file's diff: the file, what it changed, and what you can
 * do about it. Every control is optional and appears only when its handler is
 * given, so a read-only diff gets just the name and the counts.
 *
 * It draws no border and doesn't stick on its own: `DiffFile` does both when
 * it holds the header over a diff, and a virtualized viewer that places its
 * own headers does them itself.
 * @import import { DiffFileHeader } from "@gitbutler/ui-react/DiffFileHeader.tsx";
 */
export const DiffFileHeader: FC<Props> = ({
	path,
	added,
	removed,
	collapsed = false,
	onCollapsedChange,
	collapseKbd,
	collapseKbdScope,
	reviewState = "unreviewed",
	onReviewedChange,
	onMenu,
	children,
	...props
}) => {
	const lastSepIdx = path.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? path.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? path.slice(lastSepIdx + 1) : path;

	// The counts read as added/removed lines on sight, but only to someone who
	// knows the colouring: the wording carries the units, for the tooltip and for
	// screen readers alike.
	const statsParts =
		added === undefined || removed === undefined ? [] : describeLineStats(added, removed);
	const statsLabel = statsParts.length === 0 ? null : statsParts.join(", ");

	const collapseLabel = collapsed ? "Unfold" : "Fold";
	const reviewLabel = reviewLabels[reviewState];
	const hasActions = onReviewedChange !== undefined || onMenu !== undefined;

	return (
		<header {...props} className={classes(props.className, styles.header)}>
			{onCollapsedChange && (
				<Tooltip content={collapseLabel} kbd={collapseKbd} kbdScope={collapseKbdScope}>
					<Button
						size="small"
						variant="ghost"
						iconOnly
						aria-label={collapseLabel}
						aria-expanded={!collapsed}
						onClick={() => onCollapsedChange(!collapsed)}
					>
						<Icon name={collapsed ? "chevron-right" : "chevron-down"} />
					</Button>
				</Tooltip>
			)}
			<h4 className={classes("text-13", styles.path)}>
				<FileIcon fileName={fileName} />
				<span className={styles.name}>{fileName}</span>
				{directoryPath !== null && <span className={styles.directory}>{directoryPath}</span>}
			</h4>
			<div className={styles.end}>
				{children}
				{statsLabel !== null && added !== undefined && removed !== undefined && (
					<Tooltip content={statsLabel}>
						<div aria-label={statsLabel} className={styles.stats}>
							<DiffStats added={added} removed={removed} className="text-12" />
							<ChangeScale added={added} removed={removed} />
						</div>
					</Tooltip>
				)}
				{hasActions && (
					<Toolbar.Root aria-label="File actions" className={styles.actions}>
						{statsLabel !== null && <Toolbar.Separator className={styles.separator} />}
						{onReviewedChange && (
							// One button carrying checkbox semantics, with the box drawn inside it,
							// rather than a real Checkbox nested in a button or a label. Both of
							// those leave two controls where the design has one, and Base UI's
							// checkbox renders unfocusable inside a label. "Changed since you
							// reviewed it" is the mixed state; the tooltip spells that out.
							<Tooltip content={reviewLabel}>
								<Toolbar.Button
									aria-pressed={reviewState === "changed" ? "mixed" : reviewState === "reviewed"}
									className={classes(
										getButtonClassName({ size: "small", variant: "ghost" }),
										styles.review,
									)}
									onClick={() => onReviewedChange(reviewState !== "reviewed")}
								>
									<span className={styles.reviewBox} aria-hidden="true">
										{reviewState !== "unreviewed" && (
											<Icon size={10} name={reviewState === "reviewed" ? "tick" : "minus"} />
										)}
									</span>
									Reviewed
								</Toolbar.Button>
							</Tooltip>
						)}
						{onMenu && (
							<Toolbar.Button
								aria-label="File menu"
								onClick={onMenu}
								className={getButtonClassName({ size: "small", variant: "ghost", iconOnly: true })}
							>
								<Icon name="kebab" />
							</Toolbar.Button>
						)}
					</Toolbar.Root>
				)}
			</div>
		</header>
	);
};
