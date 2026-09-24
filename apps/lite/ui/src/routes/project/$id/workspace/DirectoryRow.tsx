import { FolderIcon } from "@gitbutler/ui-react/FolderIcon.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import { Toolbar, Tooltip as BaseTooltip } from "@base-ui/react";
import type { ComponentProps, FC, ReactNode } from "react";
import styles from "./FilesTree.module.css";
import fileRowStyles from "./FileRow.module.css";
import rowStyles from "./Row.module.css";
import {
	PresentationalRowButton,
	Row,
	RowCheckbox,
	RowLabel,
	RowLabelContainer,
	RowToolbar,
} from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { TreeSteps, TreeStepsToggle } from "./TreeSteps.tsx";
import type { FileRowTooltipPayload } from "./FileRowTooltip.tsx";
import { useDirectoryMenuItems } from "./useDirectoryMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";

/** Whether every file below a directory is checked, some of them, or none. */
export type DirectoryCheckedState = "checked" | "indeterminate" | "unchecked";

type DirectoryRowProps = {
	projectId: string;
	fileParent: FileParent;
	path: string;
	/** The trailing path segments this row stands for, e.g. `src/lib`. */
	name: string;
	/** Every file below this directory, in the order expanding it would reveal them. */
	items: Array<FileRowItem>;
	depth: number;
	isCollapsed: boolean;
	onToggleCollapsed: () => void;
	canCheck: boolean;
	/** Read once by the list rather than per row; see {@link FilesTree}'s prop of the same name. */
	anyOperationPending: boolean;
	checkedState: DirectoryCheckedState;
	/** Whether every change below this directory has been reviewed; the row says so, as a file row does. */
	isReviewed: boolean;
	checkDirectory: (evt: { path: string; checked: boolean }) => void;
	focusScope: FocusScope;
	tooltipHandle: BaseTooltip.Handle<FileRowTooltipPayload>;
	/** See {@link FilesTree}'s prop of the same name. */
	rail?: ReactNode;
} & ComponentProps<typeof Row>;

type DirectoryRowPresentationalProps = Omit<DirectoryRowProps, "projectId" | "fileParent"> & {
	menuItems: ReturnType<typeof useDirectoryMenuItems>;
	/** The row as it renders mid-scroll: its shape, with nothing behind it. */
	presentationalOnly?: boolean;
};

/**
 * Split from the presentational half exactly as {@link FileRow} is: the menu costs a
 * handful of queries per row, which a list being scrolled should not be paying.
 */
export const DirectoryRow: FC<DirectoryRowProps> = ({ projectId, fileParent, ...props }) => {
	const { path, items, checkedState, isReviewed, isCollapsed, onToggleCollapsed } = props;

	const menuItems = useDirectoryMenuItems({
		projectId,
		fileParent,
		path,
		items,
		checkedState,
		isReviewed,
		isCollapsed,
		onToggleCollapsed,
	});

	return <DirectoryRowPresentational {...props} menuItems={menuItems} />;
};

export const DirectoryRowPresentational: FC<DirectoryRowPresentationalProps> = ({
	path,
	name,
	items,
	depth,
	isCollapsed,
	onToggleCollapsed,
	canCheck,
	checkedState,
	isReviewed,
	checkDirectory,
	focusScope,
	tooltipHandle,
	anyOperationPending,
	menuItems,
	presentationalOnly = false,
	rail,
	...restProps
}) => (
	<Row
		{...restProps}
		isChecked={checkedState === "checked"}
		onContextMenu={
			presentationalOnly
				? undefined
				: (event) => {
						void showNativeContextMenu(event, menuItems);
					}
		}
	>
		{rail}
		<TreeSteps depth={depth}>
			<Tooltip
				disableHoverablePopup
				content={isCollapsed ? "Expand directory" : "Collapse directory"}
				kbd={changesFileHotkeys.toggleFoldDirectory.hotkey}
				kbdScope={focusScope}
			>
				<TreeStepsToggle
					isCollapsed={isCollapsed}
					aria-label={`${isCollapsed ? "Expand" : "Collapse"} directory ${path}`}
					onClick={onToggleCollapsed}
				/>
			</Tooltip>
		</TreeSteps>

		{/* The folder stands where a file's type icon stands, and gives way to the
		    checkbox on the same terms. */}
		<div className={styles.leading}>
			<FolderIcon
				className={classes(styles.leadingMark, isReviewed && fileRowStyles.reviewedFade)}
			/>
			<RowCheckbox
				disabled={anyOperationPending || !canCheck}
				aria-label={`Check directory ${path}`}
				checked={checkedState === "checked"}
				indeterminate={checkedState === "indeterminate"}
				className={styles.leadingCheckbox}
				nativeButton
				render={
					presentationalOnly ? (
						<button type="button" inert aria-hidden="true" tabIndex={-1} />
					) : (
						<BaseTooltip.Trigger
							handle={tooltipHandle}
							payload={{
								content: "Check directory",
								kbd: changesFileHotkeys.checkFile.hotkey,
								kbdScope: focusScope,
							}}
						/>
					)
				}
				onCheckedChange={
					presentationalOnly
						? undefined
						: (checked) => {
								checkDirectory({ path, checked });
							}
				}
			/>
		</div>

		{/* A folded chain names several segments at once and a deep row is narrow, so
		    the whole path is a hover away, as a file's is. */}
		<BaseTooltip.Trigger
			handle={tooltipHandle}
			payload={{ content: path }}
			render={<RowLabelContainer className={classes(isReviewed && fileRowStyles.reviewedFade)} />}
		>
			<RowLabel singleLine>{name}</RowLabel>
		</BaseTooltip.Trigger>

		{!anyOperationPending &&
			(presentationalOnly ? (
				<RowToolbar aria-hidden="true">
					<PresentationalRowButton icon="kebab" />
				</RowToolbar>
			) : (
				<Toolbar.Root aria-label="Directory actions" render={<RowToolbar />}>
					<Toolbar.Button
						aria-label="Directory menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			))}

		{/* Collapsed, the count is the only sign of what the row is holding. */}
		{isCollapsed && (
			<span className={classes(styles.fileCount, rowStyles.fadedText, "text-11")}>
				{items.length}
			</span>
		)}

		{/* The same tick a reviewed file row shows in place of its change type: every
		    change below this directory is done with. */}
		{isReviewed && (
			<BaseTooltip.Trigger
				handle={tooltipHandle}
				payload={{ content: "Reviewed" }}
				render={
					<span aria-label="Reviewed" className={fileRowStyles.reviewedMark}>
						<Icon size={11} name="tick" />
					</span>
				}
			/>
		)}
	</Row>
);
