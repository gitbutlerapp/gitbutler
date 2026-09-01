import { FolderIcon } from "@gitbutler/ui-react/FolderIcon.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { TooltipPopup } from "@gitbutler/ui-react/Tooltip.tsx";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import { Toolbar, Tooltip } from "@base-ui/react";
import type { ComponentProps, FC, ReactNode } from "react";
import styles from "./FilesTree.module.css";
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
import { useDirectoryMenuItems } from "./useDirectoryMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";

/** Whether every file below a directory is checked, some of them, or none. */
export type DirectoryCheckedState = "checked" | "indeterminate" | "unchecked";

type DirectoryRowProps = {
	projectId: string;
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
	checkDirectory: (evt: { path: string; checked: boolean }) => void;
	focusScope: FocusScope;
	/** See {@link FilesTree}'s prop of the same name. */
	rail?: ReactNode;
} & ComponentProps<typeof Row>;

type DirectoryRowPresentationalProps = Omit<DirectoryRowProps, "projectId"> & {
	menuItems: ReturnType<typeof useDirectoryMenuItems>;
	/** The row as it renders mid-scroll: its shape, with nothing behind it. */
	presentationalOnly?: boolean;
};

/**
 * Split from the presentational half exactly as {@link FileRow} is: the menu costs a
 * handful of queries per row, which a list being scrolled should not be paying.
 */
export const DirectoryRow: FC<DirectoryRowProps> = ({ projectId, ...props }) => {
	const { path, items, isCollapsed, onToggleCollapsed } = props;

	const menuItems = useDirectoryMenuItems({
		projectId,
		path,
		items,
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
	checkDirectory,
	focusScope,
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
			<Tooltip.Root disableHoverablePopup>
				<Tooltip.Trigger
					aria-label={`${isCollapsed ? "Expand" : "Collapse"} directory ${path}`}
					onClick={onToggleCollapsed}
					render={<TreeStepsToggle isCollapsed={isCollapsed} />}
				/>
				<Tooltip.Portal>
					<Tooltip.Positioner sideOffset={4}>
						<Tooltip.Popup
							render={
								<TooltipPopup
									kbd={changesFileHotkeys.toggleFoldDirectory.hotkey}
									kbdScope={focusScope}
								/>
							}
						>
							{isCollapsed ? "Expand directory" : "Collapse directory"}
						</Tooltip.Popup>
					</Tooltip.Positioner>
				</Tooltip.Portal>
			</Tooltip.Root>
		</TreeSteps>

		{/* The folder stands where a file's type icon stands, and gives way to the
		    checkbox on the same terms. */}
		<div className={styles.leading}>
			<FolderIcon className={styles.leadingMark} />
			<RowCheckbox
				disabled={anyOperationPending || !canCheck}
				aria-label={`Check directory ${path}`}
				checked={checkedState === "checked"}
				indeterminate={checkedState === "indeterminate"}
				className={styles.leadingCheckbox}
				onCheckedChange={(checked) => {
					checkDirectory({ path, checked });
				}}
			/>
		</div>

		<RowLabelContainer>
			<RowLabel singleLine>{name}</RowLabel>
		</RowLabelContainer>

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
	</Row>
);
