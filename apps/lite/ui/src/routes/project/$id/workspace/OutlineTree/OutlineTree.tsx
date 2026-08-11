import rowStyles from "../Row.module.css";
import { useCommitAmend } from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex } from "#ui/api/ref-info.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import {
	branchOperand,
	uncommittedChangesOperand,
	uncommittedChangesFileParent,
	commitOperand,
	operandIdentityKey,
	type Operand,
	operandEquals,
	commitIdentityKey,
} from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getTransferTarget } from "#ui/outline/mode.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	OperationTarget as OperationTarget_,
	type OperationTargetOutline,
} from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useOperationDropTarget } from "#ui/routes/project/$id/workspace/useOperationDropTarget.ts";
import { NavigationIndexContext } from "#ui/routes/project/$id/workspace/OutlineNavigationIndexContext.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { navigationIndexIncludes, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import { Scroller } from "#ui/components/Scroller.tsx";
import type {
	BranchReference,
	Segment,
	Stack,
	PushStatus,
	WorktreeChanges,
	WorkspaceState,
} from "@gitbutler/but-sdk";

import { useQuery } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type ComponentProps,
	createContext,
	type FC,
	Fragment,
	type ReactNode,
	use,
	useRef,
} from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";
import styles from "./OutlineTree.module.css";
import { Row, RowLabel, RowLabelContainer, SectionHeaderRow } from "../Row.tsx";
import { StackCard } from "../StackCard.tsx";
import stackCardStyles from "../StackCard.module.css";
import { treeItemId } from "../Row-utils.ts";
import { getOperation, type Placement, useDryRunOperation } from "#ui/operations/operation.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { SelectionScopeKbd } from "#ui/components/SelectionScopeKbd.tsx";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { CommitRow } from "./CommitRow.tsx";
import { BranchRow } from "./BranchRow.tsx";
import { StackRow } from "./StackRow.tsx";
import { useOutlineTreeHotkeys } from "./hotkeys.ts";
import { UncommittedChangesRow } from "./UncommittedChangesRow.tsx";
import { FileFilterRow } from "../FileFilterRow.tsx";
import { useFileFilter } from "../useFileFilter.ts";
import { getChangesFileRowItems, pathMatchesFilter, type FileRowItem } from "../file-row.ts";
import { buildFileTreeRows, type FileDisplayMode, type FileTreeRow } from "../file-tree.ts";
import { useFileDisplayMode } from "../useFileDisplayMode.ts";
import {
	canRemoveBranchReference,
	downstackPushStatusesFromSegments,
	type DownstackPushStatus,
} from "#ui/segment.ts";
import { checkedRange, navigationIndexRange } from "#ui/checking.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import {
	focusSelectionScope,
	useAutofocusSelectionScope,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import {
	CommitForm,
	type CommitTargetComboboxItem,
} from "#ui/routes/project/$id/workspace/CommitForm.tsx";
import {
	buildCommitTargetComboboxItems,
	selectCommitTargetComboboxItem,
} from "./commitTargetComboboxItems.ts";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);
DryRunWorkspaceContext.displayName = "DryRunWorkspaceContext";

const AbsorptionTargetCommitIdsContext = createContext<ReadonlySet<string> | null>(null);
AbsorptionTargetCommitIdsContext.displayName = "AbsorptionTargetCommitIdsContext";

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "uncommitted-changes-panel" | "stacks-panel";

const TreeItem: FC<
	{
		projectId: string;
		operand: Operand;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, render, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const isSelected = useAppSelector((state) =>
		projectSlice.selectors.selectIsSelectedOutline(state, projectId, navigationIndex, operand),
	);

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(operand),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

const OperationTarget: FC<
	{
		enabled: boolean;
		operand: Operand;
		projectId: string;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"button">
> = ({ enabled, operand, projectId, outline, render, ...props }) => {
	const dropRef = useOperationDropTarget({ enabled, target: operand, projectId });

	const absorptionTargetCommitIds = assert(use(AbsorptionTargetCommitIdsContext));
	const navigationIndex = assert(use(NavigationIndexContext));

	type ActiveOperation = { placement: Placement; tooltip?: string | undefined };
	const activeOperation = useAppSelector((state) => {
		const selection = projectSlice.selectors.selectSelectionOutline(
			state,
			projectId,
			navigationIndex,
		);
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
		const detailsSelectionScope = projectSlice.selectors.selectDetailsSelectionScope(
			state,
			projectId,
		);

		return Match.value(outlineMode).pipe(
			Match.tags({
				Absorb: (): ActiveOperation | null => {
					const isActive =
						operand._tag === "Commit" && absorptionTargetCommitIds.has(operand.commitId);
					if (!isActive) return null;

					return { placement: "into", tooltip: "Absorb target" };
				},
				Transfer: ({ value: mode }): ActiveOperation | null => {
					if (mode.placement === null) return null;

					const target = getTransferTarget(mode, selection, detailsSelectionScope);
					const isActive = target !== null && operandEquals(target, operand);
					if (!isActive) return null;

					return {
						placement: mode.placement,
						tooltip: getOperation({
							sources: mode.sources,
							target: operand,
							placement: mode.placement,
						})?.label,
					};
				},
			}),
			Match.orElse(() => null),
		);
	});

	return (
		<Tooltip.Root
			open={activeOperation?.tooltip !== undefined}
			disableHoverablePopup
			onOpenChange={(_, eventDetails) => {
				// Allow escape to bubble up from tree so it triggers the cancel
				// operation shortcut.
				if (eventDetails.reason === "escape-key") eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger
				{...props}
				render={
					<OperationTarget_
						ref={(el) => {
							dropRef.current = el;
						}}
						placement={activeOperation?.placement}
						outline={outline}
						render={render}
					/>
				}
			/>
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup render={<TooltipPopup />}>{activeOperation?.tooltip}</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};

const OperandC: FC<
	{
		projectId: string;
		operand: Operand;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, outline, render, ...props }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				source={operand}
				outline={outline}
				render={
					<OperationTarget
						enabled={navigationIndexIncludes(navigationIndex, operand, operandIdentityKey)}
						projectId={projectId}
						operand={operand}
						outline={outline}
						render={render}
					/>
				}
			/>
		),
		defaultTagName: "div",
		props,
	});
};

/**
 * Laid out from the same source and settings as the navigation index the page
 * builds, so the rows and the keys that move between them stay in step. Kept
 * out of the component so the compiler memoises it on those inputs.
 */
const buildUncommittedFileRows = ({
	worktreeChanges,
	filter,
	mode,
	collapsedDirectories,
}: {
	worktreeChanges: WorktreeChanges | undefined;
	filter: string | null;
	mode: FileDisplayMode;
	collapsedDirectories: Record<string, true>;
}): Array<FileTreeRow<FileRowItem>> =>
	buildFileTreeRows({
		items: (worktreeChanges ? getChangesFileRowItems(worktreeChanges) : []).filter((item) =>
			pathMatchesFilter(item.path, filter),
		),
		mode,
		collapsedDirectories,
	});

const UncommittedChanges: FC<{
	navigationIndex: NavigationIndex<string>;
	commitTarget: CommitTargetComboboxItem | null;
	projectId: string;
	targetComboboxItems: Array<CommitTargetComboboxItem>;
	hasNoBranches: boolean;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	onActiveFileSelection: (selection: string) => void;
	onEdgeSpill: (offset: -1 | 1) => void;
	worktreeChanges: WorktreeChanges | undefined;
}> = ({
	navigationIndex,
	commitTarget,
	projectId,
	targetComboboxItems,
	hasNoBranches,
	onAmendCommit,
	canAmendCommit,
	onActiveFileSelection,
	onEdgeSpill,
	worktreeChanges,
}) => {
	const dispatch = useAppDispatch();

	const filter = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesFilter(state, projectId),
	);
	const fileDisplayMode = useFileDisplayMode();
	const collapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesCollapsedDirectories(state, projectId),
	);
	const fileRows = buildUncommittedFileRows({
		worktreeChanges,
		filter,
		mode: fileDisplayMode,
		collapsedDirectories,
	});

	const fileSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUncommittedFiles(state, projectId, navigationIndex),
	);

	const panelRef = useRef<HTMLDivElement>(null);
	const fileListRef = useRef<HTMLDivElement>(null);
	const fileFilter = useFileFilter({
		filter,
		setFilter: (filter) =>
			dispatch(projectSlice.actions.setUncommittedFilesFilter({ projectId, filter })),
		inputId: "uncommitted-files-filter-input",
		scope: "uncommitted-files",
		selection: fileSelection,
		firstPath: fileRows[0]?.path,
		onEnterList: onActiveFileSelection,
		panelRef,
		listRef: fileListRef,
		enabled: (worktreeChanges?.changes.length ?? 0) > 0,
	});

	return (
		<div className={styles.uncommittedChanges} ref={panelRef}>
			{fileFilter.rowProps === null ? (
				<UncommittedChangesRow
					changes={worktreeChanges?.changes ?? []}
					projectId={projectId}
					onOpenFilter={fileFilter.open}
				/>
			) : (
				<FileFilterRow {...fileFilter.rowProps} />
			)}

			<Scroller
				withSeparator
				className={styles.uncommittedChangesTreeArea}
				viewportClassName={styles.uncommittedChangesTree}
			>
				<FilesTree
					selectionScope="uncommitted-files"
					onFocus={() =>
						dispatch(
							projectSlice.actions.setDetailsSelectionScope({
								projectId,
								scope: "uncommitted-files",
							}),
						)
					}
					emptyLabel={
						filter !== null && (worktreeChanges?.changes.length ?? 0) > 0
							? "No matching files."
							: "Nothing to commit"
					}
					fileParent={uncommittedChangesFileParent}
					rows={fileRows}
					collapsedDirectories={collapsedDirectories}
					onToggleDirectoryCollapsed={(path) =>
						dispatch(
							projectSlice.actions.toggleUncommittedFilesDirectoryCollapsed({ projectId, path }),
						)
					}
					navigationIndex={navigationIndex}
					onRowSelection={onActiveFileSelection}
					onEdgeSpill={onEdgeSpill}
					projectId={projectId}
					ref={useMergedRefs(fileListRef, useAutofocusSelectionScope())}
					selection={fileSelection}
				/>
			</Scroller>

			<CommitForm
				projectId={projectId}
				commitTarget={commitTarget}
				targetComboboxItems={targetComboboxItems}
				hasNoBranches={hasNoBranches}
				startCommitButtonId={startCommitButtonId}
				commitMessageInputId={commitMessageInputId}
				className={styles.commitForm}
				onAmendCommit={onAmendCommit}
				canAmendCommit={canAmendCommit}
				worktreeChanges={worktreeChanges}
			/>
		</div>
	);
};

const segmentPushStatusToGraphSegmentStatus = (pushStatus: PushStatus): GraphSegmentStatus => {
	switch (pushStatus) {
		case "nothingToPush":
			return "LocalAndRemote";
		case "unpushedCommits":
		case "completelyUnpushed":
			return "LocalOnly";
		case "unpushedCommitsRequiringForce":
			return "Diverged";
		case "integrated":
			return "Integrated";
	}
};

const BranchSegment: FC<{
	projectId: string;
	segment: Segment;
	refName: BranchReference;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
	downstackPushStatus: DownstackPushStatus;
	isTopSegment: boolean;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
}> = ({
	projectId,
	segment,
	refName,
	canTearOffBranch,
	canRemoveBranch,
	downstackPushStatus,
	isTopSegment,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
}) => {
	const operand = branchOperand({ branchRef: refName.fullNameBytes });

	return (
		<TreeItem
			projectId={projectId}
			operand={operand}
			aria-label={refName.displayName}
			aria-expanded
			render={<OperandC projectId={projectId} operand={operand} outline="outside" />}
		>
			<BranchRow
				projectId={projectId}
				refName={refName}
				canTearOffBranch={canTearOffBranch}
				canRemoveBranch={canRemoveBranch}
				downstackPushStatus={downstackPushStatus}
				pushStatus={segment.pushStatus}
				graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
				commitCount={segment.commits.length}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group">
				<SegmentContent
					projectId={projectId}
					segment={segment}
					checkCommit={checkCommit}
					onAmendCommit={onAmendCommit}
					canAmendCommit={canAmendCommit}
				/>
			</div>
		</TreeItem>
	);
};

const EmptySegmentContent: FC<{
	segment: Segment;
}> = ({ segment }) => {
	const navigationIndex = assert(use(NavigationIndexContext));

	const refName = assert(segment.refName);
	const inert = !navigationIndexIncludes(
		navigationIndex,
		branchOperand({ branchRef: refName.fullNameBytes }),
		operandIdentityKey,
	);

	return (
		<div>
			<Row interactive={false} inert={inert}>
				<GraphSegment
					glyph="parent"
					status={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				/>
				<RowLabelContainer>
					<RowLabel className={rowStyles.fadedText}>No commits.</RowLabel>
				</RowLabelContainer>
			</Row>
		</div>
	);
};

const SegmentContent: FC<{
	projectId: string;
	segment: Segment;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
}> = ({ projectId, segment, checkCommit, onAmendCommit, canAmendCommit }) => {
	// A plain boolean, so this re-renders only when this segment's own fold
	// state changes rather than on every fold anywhere.
	const isFolded = useAppSelector(
		(state) =>
			segment.refName !== null &&
			projectSlice.selectors.selectSegmentFolded(
				state,
				projectId,
				decodeBytes(segment.refName.fullNameBytes),
			),
	);

	if (segment.commits.length === 0) return <EmptySegmentContent segment={segment} />;
	// The branch row stands in for a folded segment: it takes the group glyph
	// and shows the count of the commits hidden here.
	if (isFolded) return null;

	const dryRunWorkspace = use(DryRunWorkspaceContext);
	const dryRunHeadInfoIndex = dryRunWorkspace ? getHeadInfoIndex(dryRunWorkspace.headInfo) : null;

	return (
		<div>
			{segment.commits.map((commit) => {
				const operand = commitOperand({ commitId: commit.id, changeId: commit.changeId });
				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunCommitId !== undefined
						? (dryRunHeadInfoIndex?.commitContextByCommitId(dryRunCommitId)?.commit ?? null)
						: null;
				return (
					<TreeItem
						key={commit.id}
						projectId={projectId}
						operand={operand}
						aria-label={commitTitle(commit.message) ?? "(no message)"}
						render={
							<OperandC
								projectId={projectId}
								operand={operand}
								outline="outside"
								render={
									<CommitRow
										commit={commit}
										checkCommit={checkCommit}
										amendCommit={() => onAmendCommit(commit.id)}
										canAmendCommit={canAmendCommit}
										projectId={projectId}
										dryRunCommit={dryRunCommit}
									/>
								}
							/>
						}
					/>
				);
			})}
		</div>
	);
};

const StackC: FC<{
	projectId: string;
	stack: Stack;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
}> = ({ projectId, stack, checkCommit, onAmendCommit, canAmendCommit }) => {
	const canTearOffBranch = stack.segments.length > 1;
	const downstackPushStatuses = downstackPushStatusesFromSegments(stack.segments);
	const navigationIndex = assert(use(NavigationIndexContext));

	return (
		<StackCard
			// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- This is a group of treeitems.
			role="group"
			aria-label="Stack"
			header={<StackRow projectId={projectId} stack={stack} />}
			bodyClassName={styles.segments}
		>
			{stack.segments.map((segment, index) => {
				const downstackPushStatus = assert(downstackPushStatuses[index]);

				const key = segment.refName
					? JSON.stringify(segment.refName.fullNameBytes)
					: // A segment should always either have a branch reference or at
						// least one commit.
						assert(segment.commits[0]).id;

				return (
					<Fragment key={key}>
						<div>
							{segment.refName ? (
								<BranchSegment
									projectId={projectId}
									segment={segment}
									refName={segment.refName}
									canTearOffBranch={canTearOffBranch}
									canRemoveBranch={canRemoveBranchReference(stack, index)}
									downstackPushStatus={downstackPushStatus}
									isTopSegment={index === 0}
									checkCommit={checkCommit}
									onAmendCommit={onAmendCommit}
									canAmendCommit={canAmendCommit}
								/>
							) : (
								<SegmentContent
									projectId={projectId}
									segment={segment}
									checkCommit={checkCommit}
									onAmendCommit={onAmendCommit}
									canAmendCommit={canAmendCommit}
								/>
							)}
						</div>
						<Row
							interactive={false}
							className={stackCardStyles.railConnector}
							inert={
								!navigationIndexIncludes(
									navigationIndex,
									segment.commits.length === 0
										? branchOperand({ branchRef: assert(segment.refName).fullNameBytes })
										: commitOperand({
												commitId: assert(segment.commits.at(-1)).id,
												changeId: assert(segment.commits.at(-1)).changeId,
											}),
									operandIdentityKey,
								)
							}
						>
							<GraphSegment
								glyph="parent"
								status={
									segment.commits.length === 0
										? segmentPushStatusToGraphSegmentStatus(segment.pushStatus)
										: commitIsDiverged(assert(segment.commits.at(-1)))
											? "Diverged"
											: assert(segment.commits.at(-1)).state.type
								}
							/>
						</Row>
					</Fragment>
				);
			})}
		</StackCard>
	);
};

const startCommitButtonId = "start-commit-button";
const commitMessageInputId = "commit-message-input";

const focusCommitMessageInput = () => {
	const input = document.getElementById(commitMessageInputId);
	if (input) input.focus();
	// The commit form may be collapsed; clicking the trigger expands it and
	// focuses the message input.
	else document.getElementById(startCommitButtonId)?.click();
};

const Stacks: FC<{
	projectId: string;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	onEdgeSpill: (offset: -1 | 1) => void;
}> = ({ projectId, checkCommit, onAmendCommit, canAmendCommit, onEdgeSpill }) => {
	const navigationIndex = assert(use(NavigationIndexContext));
	const dispatch = useAppDispatch();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, navigationIndex),
	);
	const dryRunOperation = useAppSelector((state) => {
		const outlineMode = projectSlice.selectors.selectOutlineModeState(state, projectId);
		const detailsSelectionScope = projectSlice.selectors.selectDetailsSelectionScope(
			state,
			projectId,
		);

		return Match.value(outlineMode).pipe(
			Match.tags({
				Transfer: ({ value: mode }) => {
					if (mode.placement === null) return;

					const target = getTransferTarget(mode, selection, detailsSelectionScope);
					if (!target) return;

					return getOperation({
						sources: mode.sources,
						target,
						placement: mode.placement,
					})?.operation;
				},
			}),
			Match.orElse(() => undefined),
		);
	});

	// TODO: debounce?
	const { data: dryRunOperationResult } = useDryRunOperation({
		projectId,
		operation: dryRunOperation,
	});
	const dryRunWorkspace = dryRunOperationResult?.workspace ?? null;

	const hotkeysRef = useRef<HTMLDivElement>(null);
	useOutlineTreeHotkeys({
		navigationIndex,
		projectId,
		ref: hotkeysRef,
		checkCommit,
		focusCommitMessageInput,
		onEdgeSpill,
	});

	return (
		<DryRunWorkspaceContext value={dryRunWorkspace}>
			<div
				tabIndex={0}
				role="tree"
				aria-activedescendant={selection ? treeItemId(selection) : undefined}
				className={classes(styles.tree, styles.stacks)}
				data-selection-scope={"outline" satisfies SelectionScope}
				onFocus={() =>
					dispatch(projectSlice.actions.setDetailsSelectionScope({ projectId, scope: "outline" }))
				}
				ref={hotkeysRef}
			>
				{(headInfo?.stacks.toReversed() ?? []).map((stack) => (
					<StackC
						key={stack.id}
						projectId={projectId}
						stack={stack}
						checkCommit={checkCommit}
						onAmendCommit={onAmendCommit}
						canAmendCommit={canAmendCommit}
					/>
				))}
			</div>
		</DryRunWorkspaceContext>
	);
};

export const OutlineTree: FC<
	{
		projectId: string;
		navigationIndex: NavigationIndex<Operand>;
		uncommittedFilesNavigationIndex: NavigationIndex<string>;
		absorptionTargetCommitIds: ReadonlySet<string>;
		onActiveFileSelection: (selection: string) => void;
		stacksHeaderActions?: ReactNode;
	} & ComponentProps<"div">
> = ({
	projectId,
	navigationIndex,
	uncommittedFilesNavigationIndex,
	absorptionTargetCommitIds,
	onActiveFileSelection,
	stacksHeaderActions,
	...props
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;

	const outlineSelection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionOutline(state, projectId, navigationIndex),
	);
	const commitTargetComboboxItems = buildCommitTargetComboboxItems({
		headInfo,
		headInfoIndex,
		outlineSelection,
	});
	const commitTarget = selectCommitTargetComboboxItem({
		items: commitTargetComboboxItems,
		outlineSelection,
	});
	// Undefined `headInfo` is still loading, which is not the same as "empty" —
	// treating it as empty would flash the draft-branch affordance on every open.
	const hasNoBranches = headInfo !== undefined && headInfo.stacks.length === 0;
	const store = useAppStore();
	const dispatch = useAppDispatch();
	const { isPending: isCommitAmendPending, mutate: commitAmend } = useCommitAmend();
	const canAmendCommit =
		!isCommitAmendPending && !!worktreeChanges && worktreeChanges.changes.length > 0;
	const amendCommit = (commitId: string) => {
		if (!worktreeChanges) return;

		const checkedUncommittedFilePaths = projectSlice.selectors.selectCheckedUncommittedFilePaths(
			store.getState(),
			projectId,
		);
		commitAmend({
			projectId,
			commitId,
			changes: worktreeChanges.changes.flatMap((change) =>
				checkedUncommittedFilePaths.size === 0 || checkedUncommittedFilePaths.has(change.path)
					? [createDiffSpec(change, [])]
					: [],
			),
			changesSource: { type: "head" },
			dryRun: false,
		});
	};

	const commitCheckRangeAnchor = useRef<string>(null);
	const commitCheckRangeEnd = useRef<string>(null);

	const rangeResolver = navigationIndexRange<Operand, string>({
		navigationIndex,
		getKey: (commitId) => commitIdentityKey({ commitId }),
		filterMap: (item) => (item._tag === "Commit" ? item.commitId : null),
	});
	const getCheckedRange = checkedRange(rangeResolver);

	const checkCommit = ({ commitId, shiftKey }: { commitId: string; shiftKey: boolean }): void => {
		const checkedCommitIds = projectSlice.selectors.selectCheckedCommitIds(
			store.getState(),
			projectId,
		);
		const nextCommitRange = getCheckedRange({
			checked: checkedCommitIds,
			rangeAnchor: commitCheckRangeAnchor.current,
			rangeEnd: commitCheckRangeEnd.current,
		})({
			item: commitId,
			shiftKey,
		});

		commitCheckRangeAnchor.current = nextCommitRange.rangeAnchor;
		commitCheckRangeEnd.current = nextCommitRange.rangeEnd;

		const checkedCommits = nextCommitRange.checked.difference(checkedCommitIds);
		const uncheckedCommits = checkedCommitIds.difference(nextCommitRange.checked);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(checkedCommits).flatMap((commitId) => {
					const ctx = headInfoIndex?.commitContextByCommitId(commitId);
					return ctx ? commitOperand({ commitId, changeId: ctx.commit.changeId }) : [];
				}),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkOperands({
				projectId,
				operands: Array.from(uncheckedCommits).flatMap((commitId) => {
					const ctx = headInfoIndex?.commitContextByCommitId(commitId);
					return ctx ? commitOperand({ commitId, changeId: ctx.commit.changeId }) : [];
				}),
				checked: false,
			}),
		);
	};

	const layoutId = `project=${projectId}:outline-tree`;
	const outlineLayout = useDefaultLayout({
		id: layoutId,
		panelIds: ["uncommitted-changes-panel", "stacks-panel"] satisfies Array<PanelId>,
	});

	// The two panes stack vertically, so arrow navigation continues across
	// their boundary: entering a pane selects its item nearest to the border,
	// while the pane being left keeps its selection. An empty neighbor keeps
	// focus where it is. Mod+Alt+arrow pane toggling stays selection-neutral.
	const spillIntoStacks = (offset: -1 | 1) => {
		if (offset !== 1) return;
		const item = navigationIndex.items.at(0);
		if (item === undefined) return;
		dispatch(projectSlice.actions.selectOutline({ projectId, selection: item }));
		focusSelectionScope("outline");
	};
	const spillIntoUncommittedChanges = (offset: -1 | 1) => {
		if (offset !== -1) return;
		const path = uncommittedFilesNavigationIndex.items.at(-1);
		if (path === undefined) return;
		onActiveFileSelection(path);
		focusSelectionScope("uncommitted-files");
	};

	return (
		<NavigationIndexContext value={navigationIndex}>
			<AbsorptionTargetCommitIdsContext value={absorptionTargetCommitIds}>
				<Group
					{...props}
					id={layoutId}
					orientation="vertical"
					className={classes(props.className, styles.tree)}
					defaultLayout={outlineLayout.defaultLayout}
					onLayoutChanged={outlineLayout.onLayoutChanged}
				>
					<Panel
						id={"uncommitted-changes-panel" satisfies PanelId}
						className={styles.uncommittedChangesOuterPanel}
						defaultSize={280}
						minSize={200}
						groupResizeBehavior="preserve-pixel-size"
					>
						<OperationSourceC
							projectId={projectId}
							source={uncommittedChangesOperand}
							outline="inside"
							render={
								<OperationTarget
									enabled
									projectId={projectId}
									operand={uncommittedChangesOperand}
									outline="inside"
									render={
										<UncommittedChanges
											navigationIndex={uncommittedFilesNavigationIndex}
											commitTarget={commitTarget}
											projectId={projectId}
											targetComboboxItems={commitTargetComboboxItems}
											hasNoBranches={hasNoBranches}
											onAmendCommit={amendCommit}
											canAmendCommit={canAmendCommit}
											onActiveFileSelection={onActiveFileSelection}
											onEdgeSpill={spillIntoStacks}
											worktreeChanges={worktreeChanges}
										/>
									}
								/>
							}
						/>
					</Panel>

					<ResizeHandle />

					<Panel id={"stacks-panel" satisfies PanelId} className={styles.stacksPanel} minSize={120}>
						<SectionHeaderRow
							label="Stacks and branches"
							childrenBefore={<SelectionScopeKbd hotkey="2" scope="outline" />}
							className={styles.stacksHeader}
							actions={stacksHeaderActions}
						/>

						<Scroller
							withSeparator
							className={styles.stacksScroller}
							viewportClassName={styles.stacksViewport}
						>
							<Stacks
								projectId={projectId}
								checkCommit={checkCommit}
								onAmendCommit={amendCommit}
								canAmendCommit={canAmendCommit}
								onEdgeSpill={spillIntoUncommittedChanges}
							/>
						</Scroller>
					</Panel>
				</Group>
			</AbsorptionTargetCommitIdsContext>
		</NavigationIndexContext>
	);
};
