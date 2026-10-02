import { Checkbox } from "@gitbutler/ui-react/Checkbox.tsx";
import { FileListItem } from "@gitbutler/ui-react/FileList.tsx";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
import { useAppSelector } from "#ui/store.ts";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { Toolbar, Tooltip } from "@base-ui/react";
import type { ComponentProps, FC } from "react";
import { PresentationalRowButton } from "./Row.tsx";
import { getRowButtonClassName, rowPointerProps } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";
import type { FileRowTooltipPayload } from "./FileRowTooltip.tsx";

type FileRowProps = {
	item: FileRowItem;
	projectId: string;
	fileParent: FileParent;
	branchNameByCommitId: (commitId: string) => string | undefined;
	canCheck: boolean;
	isChecked: boolean;
	isIndeterminate?: boolean;
	/** Whether the diff on show has been reviewed; the row says so in place of its change type. */
	isReviewed: boolean;
	checkFile: (evt: { path: string; shiftKey: boolean }) => void;
	/** How many directories this row sits inside. Zero in list mode. */
	depth: number;
	/**
	 * Where the directory goes: leading the file name, trailing it, or nowhere
	 * — the tree already says which directory this is. Resolved by the list.
	 */
	pathDisplay: "lead" | "trail" | "hidden";
	focusScope: FocusScope;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
	isSelected: boolean;
	onSelect: () => void;
} & ComponentProps<"div">;

type FileRowPresentationalProps = FileRowProps & {
	anyOperationPending: boolean;
	menuItems: ReturnType<typeof useFileMenuItems>;
	presentationalOnly?: boolean;
};

export const FileRow: FC<FileRowProps> = (props) => {
	const { item, projectId, fileParent, isReviewed } = props;
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const anyOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag !== "None",
	);
	const menuItems = useFileMenuItems({
		projectId,
		address: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
		isReviewed,
	});

	return (
		<FileRowPresentational
			{...props}
			anyOperationPending={anyOperationPending}
			menuItems={menuItems}
		/>
	);
};

export const FileRowPresentational: FC<FileRowPresentationalProps> = ({
	item,
	projectId,
	fileParent,
	branchNameByCommitId,
	canCheck,
	isChecked,
	isIndeterminate = false,
	isReviewed,
	checkFile,
	depth,
	pathDisplay,
	focusScope,
	anyOperationPending,
	menuItems,
	presentationalOnly = false,
	tooltipHandle,
	isSelected,
	onSelect,
	...restProps
}) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const hasConflictHint = item._tag === "Conflict" && fileParent._tag === "UncommittedChanges";
	// An uncommitted conflict is a state to get out of, so the row says how.
	const rowTooltip = hasConflictHint
		? `${relativePath} — Resolve the conflict, then right-click → Mark as Resolved`
		: relativePath;
	const lastSepIdx = relativePath.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;

	return (
		<FileListItem
			{...restProps}
			{...rowPointerProps({
				...restProps,
				onSelect,
				onShiftSelect:
					!presentationalOnly && !anyOperationPending && canCheck
						? () => checkFile({ path: relativePath, shiftKey: true })
						: undefined,
			})}
			name={fileName}
			directory={pathDisplay === "hidden" ? null : directoryPath}
			directoryPosition={pathDisplay === "lead" ? "lead" : "trail"}
			status={item._tag === "Change" ? item.change.status.type : undefined}
			reviewed={isReviewed}
			conflicted={item._tag === "Conflict"}
			selected={isSelected}
			depth={depth}
			checkbox={
				<Checkbox
					disabled={anyOperationPending || !canCheck}
					aria-label={`Check file ${relativePath}`}
					checked={isChecked}
					indeterminate={isIndeterminate}
					nativeButton
					render={
						presentationalOnly ? (
							<button type="button" inert aria-hidden="true" tabIndex={-1} />
						) : (
							<Tooltip.Trigger
								handle={tooltipHandle}
								payload={{
									content: changesFileHotkeys.checkFile.meta.name,
									kbd: changesFileHotkeys.checkFile.hotkey,
									kbdScope: focusScope,
								}}
							/>
						)
					}
					onCheckedChange={
						presentationalOnly
							? undefined
							: (_checked, { event }) => {
									const shiftKey =
										(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
										event.shiftKey === true;
									checkFile({ path: relativePath, shiftKey });
								}
					}
				/>
			}
			labelRender={
				<Tooltip.Trigger
					handle={tooltipHandle}
					payload={{ content: rowTooltip }}
					render={<div />}
				/>
			}
			statusRender={
				<Tooltip.Trigger
					handle={tooltipHandle}
					payload={{
						content:
							item._tag === "Conflict"
								? "Conflicted"
								: isReviewed
									? "Reviewed"
									: item.change.status.type,
					}}
					// A tooltip trigger is a button by default, and the status isn't one.
					render={<span />}
				/>
			}
			actions={
				anyOperationPending ? undefined : presentationalOnly ? (
					<PresentationalRowButton icon="kebab" />
				) : (
					<Toolbar.Root aria-label="File actions">
						<Toolbar.Button
							aria-label="File menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(
									event.currentTarget,
									menuItems,
									fileParent._tag === "UncommittedChanges" ? { path: relativePath } : undefined,
								);
							}}
							className={getRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)
			}
			marks={
				!anyOperationPending &&
				item._tag === "Change" &&
				fileParent._tag === "UncommittedChanges" &&
				item.dependencyCommitIds.length > 0 ? (
					presentationalOnly ? (
						<PresentationalRowButton icon="link" />
					) : (
						<Toolbar.Root aria-label="File actions">
							<Toolbar.Button
								render={
									<DependencyIndicator
										projectId={projectId}
										commitIds={item.dependencyCommitIds}
										branchNameByCommitId={branchNameByCommitId}
										tooltipHandle={tooltipHandle}
										className={getRowButtonClassName({ iconOnly: true })}
									/>
								}
							>
								<Icon name="link" />
							</Toolbar.Button>
						</Toolbar.Root>
					)
				) : undefined
			}
			onContextMenu={
				presentationalOnly
					? undefined
					: (event) => {
							// Hand the file path along so a plugin host can add its own
							// actions (the app's native menus ignore it).
							void showNativeContextMenu(
								event,
								menuItems,
								fileParent._tag === "UncommittedChanges" ? { path: relativePath } : undefined,
							);
						}
			}
		/>
	);
};
