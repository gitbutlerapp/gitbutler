import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import rowStyles from "./Row.module.css";
import { showNativeContextMenu, showNativeMenuFromTrigger } from "#ui/native-menu.ts";
import type { FileParent } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import type { FocusScope } from "#ui/focus-scopes.ts";
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
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";
import { TreeSteps } from "./TreeSteps.tsx";
import type { TreeChange } from "@gitbutler/but-sdk";
import type { FileRowTooltipPayload } from "./FileRowTooltip.tsx";

type FileRowProps = {
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
	focusScope: FocusScope;
	tooltipHandle: Tooltip.Handle<FileRowTooltipPayload>;
} & Omit<ComponentProps<typeof Row>, "projectId">;

type FileRowPresentationalProps = Omit<FileRowProps, "canUncommit" | "uncommit"> & {
	anyOperationPending: boolean;
	menuItems: ReturnType<typeof useFileMenuItems>;
};

export const FileRow: FC<FileRowProps> = ({ canUncommit, uncommit, ...props }) => {
	const { item, projectId, fileParent } = props;
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const anyOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag !== "None",
	);
	const menuItems = useFileMenuItems({
		projectId,
		address: { parent: fileParent, path: relativePath },
		path: relativePath,
		change: item._tag === "Change" ? item.change : undefined,
		canUncommit,
		uncommit,
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
	checkFile,
	depth,
	pathDisplay,
	focusScope,
	anyOperationPending,
	menuItems,
	tooltipHandle,
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
		<Row
			{...restProps}
			isChecked={isChecked}
			onShiftSelect={
				!anyOperationPending && canCheck
					? () => checkFile({ path: relativePath, shiftKey: true })
					: undefined
			}
			onContextMenu={(event) => {
				// Hand the file path along so a plugin host can add its own
				// actions (the app's native menus ignore it).
				void showNativeContextMenu(
					event,
					menuItems,
					fileParent._tag === "UncommittedChanges" ? { path: relativePath } : undefined,
				);
			}}
		>
			<TreeSteps depth={depth} />

			<div className={treeStyles.leading}>
				<FileIcon fileName={fileName} className={treeStyles.leadingMark} />
				<RowCheckbox
					disabled={anyOperationPending || !canCheck}
					aria-label={`Check file ${relativePath}`}
					checked={isChecked}
					className={treeStyles.leadingCheckbox}
					nativeButton
					render={
						<Tooltip.Trigger
							handle={tooltipHandle}
							payload={{
								content: changesFileHotkeys.checkFile.meta.name,
								kbd: changesFileHotkeys.checkFile.hotkey,
								kbdScope: focusScope,
							}}
						/>
					}
					onCheckedChange={(_checked, { event }) => {
						const shiftKey =
							(event instanceof MouseEvent || event instanceof KeyboardEvent) &&
							event.shiftKey === true;
						checkFile({ path: relativePath, shiftKey });
					}}
				/>
			</div>

			<Tooltip.Trigger
				handle={tooltipHandle}
				payload={{ content: rowTooltip }}
				render={<RowLabelContainer />}
			>
				{item._tag === "Conflict" && (
					<ConflictIcon
						variant="conflict"
						className={styles.conflictIcon}
						aria-label="Conflicted"
					/>
				)}
				<RowLabel singleLine>
					{directoryPath !== null && pathDisplay === "lead" && (
						<span className={classes(styles.pathLead, rowStyles.fadedText)}>{directoryPath}/</span>
					)}
					{fileName}
					{directoryPath !== null && pathDisplay === "trail" && (
						<span className={classes(styles.pathInit, rowStyles.fadedText)}>{directoryPath}</span>
					)}
				</RowLabel>
			</Tooltip.Trigger>

			{!anyOperationPending && (
				<Toolbar.Root aria-label="File actions" render={<RowToolbar />}>
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
			)}

			{!anyOperationPending &&
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
									tooltipHandle={tooltipHandle}
									className={getRowButtonClassName({ iconOnly: true })}
								/>
							}
						>
							<Icon name="link" />
						</Toolbar.Button>
					</Toolbar.Root>
				)}

			{item._tag === "Change" && (
				<Tooltip.Trigger
					handle={tooltipHandle}
					payload={{ content: item.change.status.type }}
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
			)}
		</Row>
	);
};
