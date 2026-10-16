import { Checkbox } from "@gitbutler/ui-react/Checkbox.tsx";
import { FileListItem } from "@gitbutler/ui-react/FileList.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import { Toolbar, Tooltip as BaseTooltip } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import { PresentationalRowButton } from "./Row.tsx";
import { getRowButtonClassName, rowPointerProps } from "./Row-utils.ts";
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
	isSelected: boolean;
	onSelect: () => void;
} & ComponentProps<"div">;

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
	isSelected,
	onSelect,
	...restProps
}) => (
	<FileListItem
		{...restProps}
		{...rowPointerProps({ ...restProps, onSelect })}
		name={name}
		reviewed={isReviewed}
		selected={isSelected}
		depth={depth}
		folded={isCollapsed}
		onToggleFolded={onToggleCollapsed}
		toggleRender={
			<BaseTooltip.Trigger
				handle={tooltipHandle}
				payload={{
					content: isCollapsed ? "Expand directory" : "Collapse directory",
					kbd: changesFileHotkeys.toggleFoldDirectory.hotkey,
					kbdScope: focusScope,
				}}
				aria-label={`${isCollapsed ? "Expand" : "Collapse"} directory ${path}`}
			/>
		}
		checkbox={
			<Checkbox
				disabled={anyOperationPending || !canCheck}
				aria-label={`Check directory ${path}`}
				checked={checkedState === "checked"}
				indeterminate={checkedState === "indeterminate"}
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
		}
		// A folded chain names several segments at once and a deep row is narrow,
		// so the whole path is a hover away, as a file's is.
		labelRender={
			<BaseTooltip.Trigger handle={tooltipHandle} payload={{ content: path }} render={<div />} />
		}
		statusRender={
			<BaseTooltip.Trigger
				handle={tooltipHandle}
				payload={{
					content: isReviewed
						? "Reviewed"
						: `${items.length} ${items.length === 1 ? "file" : "files"}`,
				}}
				render={<span />}
			/>
		}
		actions={
			anyOperationPending ? undefined : presentationalOnly ? (
				<PresentationalRowButton icon="kebab" />
			) : (
				<Toolbar.Root aria-label="Directory actions">
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
			)
		}
		// Folded, the count is the only sign of what the row is holding.
		count={isCollapsed ? items.length : undefined}
		onContextMenu={
			presentationalOnly
				? undefined
				: (event) => {
						void showNativeContextMenu(event, menuItems);
					}
		}
	/>
);
