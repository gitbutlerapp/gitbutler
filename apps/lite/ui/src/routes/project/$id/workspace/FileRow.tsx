import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import rowStyles from "./Row.module.css";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { SelectionScope } from "#ui/selection-scopes.ts";
import { useAppSelector } from "#ui/store.ts";
import { Icon } from "#ui/components/Icon.tsx";
import { classes } from "#ui/components/classes.ts";
import { changesFileHotkeys } from "#ui/hotkeys.ts";
import { Toolbar, Tooltip } from "@base-ui/react";
import { Match } from "effect";
import type { ComponentProps, FC } from "react";
import styles from "./FileRow.module.css";
import treeStyles from "./FilesTree.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";
import { TreeSteps } from "./TreeSteps.tsx";
import type { TreeChange } from "@gitbutler/but-sdk";

export const FileRow: FC<
	{
		item: FileRowItem;
		projectId: string;
		fileParent: FileParent;
		branchNameByCommitId: (commitId: string) => string | undefined;
		canCheck: boolean;
		canUncommit: boolean;
		uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
		isChecked: boolean;
		checkFile: (evt: { path: string; shiftKey: boolean }) => void;
		/** How many directories this row sits inside. Zero in list mode. */
		depth: number;
		/**
		 * Where the directory goes: leading the file name, trailing it, or nowhere
		 * — the tree already says which directory this is. Resolved by the list.
		 */
		pathDisplay: "lead" | "trail" | "hidden";
		selectionScope: SelectionScope;
	} & Omit<ComponentProps<typeof Row>, "projectId">
> = ({
	item,
	projectId,
	fileParent,
	branchNameByCommitId,
	canCheck,
	canUncommit,
	uncommit,
	isChecked,
	checkFile,
	depth,
	pathDisplay,
	selectionScope,
	id,
	...restProps
}) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const isDefaultMode = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineModeState(state, projectId)._tag === "Default",
	);
	const menuItems = useFileMenuItems({
		projectId,
		operand: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
		canUncommit,
		uncommit,
	});

	const lastSepIdx = relativePath.lastIndexOf("/");
	const directoryPath = lastSepIdx !== -1 ? relativePath.slice(0, lastSepIdx) : null;
	const fileName = lastSepIdx !== -1 ? relativePath.slice(lastSepIdx + 1) : relativePath;

	return (
		<Tooltip.Root disableHoverablePopup>
			<Tooltip.Trigger
				// We pass the ID here instead of including it with the other props as a
				// workaround for Base UI issue:
				// https://github.com/mui/base-ui/issues/5108
				id={id}
				render={
					<Row
						{...restProps}
						isChecked={isChecked}
						onShiftSelect={
							isDefaultMode && canCheck
								? () => checkFile({ path: relativePath, shiftKey: true })
								: undefined
						}
						className={classes(restProps.className, treeStyles.row)}
						onContextMenu={(event) => {
							void showNativeContextMenu(event, menuItems);
						}}
					/>
				}
			>
				<TreeSteps depth={depth} />

				<div className={treeStyles.leading}>
					<FileIcon fileName={fileName} className={treeStyles.leadingMark} />
					<Tooltip.Root
						// This gets in the way when the user tries to move their hover to a
						// sibling row.
						disableHoverablePopup
					>
						<RowCheckbox
							disabled={!isDefaultMode || !canCheck}
							aria-label={`Check file ${relativePath}`}
							checked={isChecked}
							className={treeStyles.leadingCheckbox}
							nativeButton
							render={<Tooltip.Trigger />}
							onCheckedChange={(_checked, { event }) => {
								const shiftKey =
									(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
									event.shiftKey === true;
								checkFile({ path: relativePath, shiftKey });
							}}
						/>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup
									render={
										<TooltipPopup
											kbd={changesFileHotkeys.checkFile.hotkey}
											kbdScope={selectionScope}
										/>
									}
								>
									{changesFileHotkeys.checkFile.meta.name}
								</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				</div>

				<RowLabelContainer>
					{item._tag === "Conflict" && (
						<ConflictIcon
							variant="conflict"
							className={styles.conflictIcon}
							aria-label="Conflicted"
						/>
					)}
					<RowLabel singleLine>
						{directoryPath !== null && pathDisplay === "lead" && (
							<span className={classes(styles.pathLead, rowStyles.fadedText)}>
								{directoryPath}/
							</span>
						)}
						{fileName}
						{directoryPath !== null && pathDisplay === "trail" && (
							<span className={classes(styles.pathInit, rowStyles.fadedText)}>{directoryPath}</span>
						)}
					</RowLabel>
				</RowLabelContainer>

				{isDefaultMode && (
					<Toolbar.Root aria-label="File actions" render={<RowToolbar />}>
						<Toolbar.Button
							aria-label="File menu"
							onClick={(event) => {
								void showNativeMenuFromTrigger(event.currentTarget, menuItems);
							}}
							className={getRowButtonClassName({ iconOnly: true })}
						>
							<Icon name="kebab" />
						</Toolbar.Button>
					</Toolbar.Root>
				)}

				{isDefaultMode &&
					item._tag === "Change" &&
					fileParent._tag === "UncommittedChanges" &&
					item.dependencyCommitIds.length > 0 && (
						<Toolbar.Root aria-label="File actions" render={<RowToolbar forceVisible />}>
							<Toolbar.Button
								render={
									<DependencyIndicator
										projectId={projectId}
										commitIds={item.dependencyCommitIds}
										branchNameByCommitId={branchNameByCommitId}
										className={getRowButtonClassName({ iconOnly: true })}
									/>
								}
							>
								<Icon name="link" />
							</Toolbar.Button>
						</Toolbar.Root>
					)}

				{item._tag === "Change" && (
					<Tooltip.Root disableHoverablePopup>
						<Tooltip.Trigger
							className={styles.statusBadge}
							aria-label={item.change.status.type}
							data-status-type={item.change.status.type}
							// By default it's a button, but we don't want this to be
							// interactive.
							render={<span />}
						>
							{Match.value(item.change.status).pipe(
								Match.when({ type: "Addition" }, () => "A"),
								Match.when({ type: "Deletion" }, () => "D"),
								Match.when({ type: "Modification" }, () => "M"),
								Match.when({ type: "Rename" }, () => "R"),
								Match.exhaustive,
							)}
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Positioner sideOffset={4}>
								<Tooltip.Popup render={<TooltipPopup />}>{item.change.status.type}</Tooltip.Popup>
							</Tooltip.Positioner>
						</Tooltip.Portal>
					</Tooltip.Root>
				)}
			</Tooltip.Trigger>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={4}>
					<Tooltip.Popup render={<TooltipPopup />}>{relativePath}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
