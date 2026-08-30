import { ConflictIcon } from "#ui/components/ConflictIcon.tsx";
import { FileIcon } from "#ui/components/FileIcon.tsx";
import { FileStatusBadge } from "#ui/components/FileStatusBadge.tsx";
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
import type { ComponentProps, CSSProperties, FC } from "react";
import styles from "./FileRow.module.css";
import treeStyles from "./FilesTree.module.css";
import { Row, RowCheckbox, RowLabel, RowLabelContainer, RowToolbar } from "./Row.tsx";
import { getRowButtonClassName } from "./Row-utils.ts";
import { DependencyIndicator } from "#ui/routes/project/$id/workspace/DependencyIndicator.tsx";
import { useFileMenuItems } from "#ui/routes/project/$id/workspace/useFileMenuItems.ts";
import type { FileRowItem } from "./file-row.ts";
import { TreeSteps } from "./TreeSteps.tsx";
import { ageBadgeOpacity, formatAgeBadge, formatRelativeTime } from "#ui/time.ts";
import type { TreeChange } from "@gitbutler/but-sdk";
import type { FileRowTooltipPayload } from "./FileRowTooltip.tsx";

/** Pulse lifetime. The 30s clock driving it can stretch this by up to a tick. */
const FRESH_CHANGE_MAX_AGE_MS = 60_000;

/** From this age up, the badge is coarse or hidden, so hover carries the full time ago. */
const AGE_TOOLTIP_MIN_AGE_MS = 60 * 60_000;

type FileRowProps = {
	item: FileRowItem;
	projectId: string;
	fileParent: FileParent;
	branchNameByCommitId: (commitId: string) => string | undefined;
	canCheck: boolean;
	canUncommit: boolean;
	uncommit?: (change: TreeChange, extendToCheckedFiles: boolean) => void;
	isChecked: boolean;
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
	/** See {@link FilesTree}'s prop of the same name. */
	ageBadgeNow?: number | null;
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
	isReviewed,
	checkFile,
	depth,
	pathDisplay,
	focusScope,
	anyOperationPending,
	menuItems,
	tooltipHandle,
	ageBadgeNow = null,
	...restProps
}) => {
	const relativePath = item._tag === "Change" ? item.change.path : item.path;

	const modifiedAtMs = item.modifiedAtMs ?? null;
	const ageMs =
		ageBadgeNow !== null && modifiedAtMs !== null ? Math.max(0, ageBadgeNow - modifiedAtMs) : null;
	const ageBadge = ageMs === null ? null : formatAgeBadge(ageMs);
	const isFresh = ageMs !== null && ageMs <= FRESH_CHANGE_MAX_AGE_MS;
	const agedTooltip =
		ageBadgeNow !== null &&
		modifiedAtMs !== null &&
		ageMs !== null &&
		ageMs > AGE_TOOLTIP_MIN_AGE_MS
			? formatRelativeTime(modifiedAtMs, ageBadgeNow)
			: null;

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
			className={classes(
				restProps.className,
				isFresh && styles.freshChange,
				isReviewed && styles.reviewedRow,
			)}
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
				payload={{
					content: agedTooltip !== null ? `${rowTooltip} — ${agedTooltip}` : rowTooltip,
				}}
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

			{ageBadge !== null && ageMs !== null && (
				<span
					className={styles.ageBadge}
					style={{ "--age-badge-opacity": String(ageBadgeOpacity(ageMs)) } as CSSProperties}
				>
					{ageBadge}
				</span>
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
					payload={{ content: isReviewed ? "Reviewed" : item.change.status.type }}
					// By default it's a button, but we don't want this to be
					// interactive.
					render={
						isReviewed ? (
							// The tick stands in for the change type rather than joining it: a
							// reviewed file's news is that it is done with, and the type is a
							// hover away.
							<span aria-label="Reviewed" className={styles.reviewedMark}>
								<Icon size={11} name="tick" />
							</span>
						) : (
							<FileStatusBadge status={item.change.status.type} />
						)
					}
				/>
			)}
		</Row>
	);
};
