import rowStyles from "../Row.module.css";
import { setCursor, useIsCursorAt, useSelection, useActiveList } from "#ui/use-cursor.ts";
import { useCommitAmend } from "#ui/api/mutations.ts";
import { changesInWorktreeQueryOptions, headInfoQueryOptions } from "#ui/api/queries.ts";
import { getHeadInfoIndex, recordedPullRequest } from "#ui/api/ref-info.ts";
import { decodeBytes } from "#ui/api/bytes.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import {
	branchAddress,
	uncommittedChangesAddress,
	uncommittedChangesFileParent,
	commitAddress,
	addressIdentityKey,
	type Address,
	addressEquals,
	commitIdentityKey,
} from "#ui/addresses.ts";
import { useReviewedPaths } from "#ui/routes/project/$id/workspace/reviewed-paths.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getTransferKind, getTransferTarget } from "#ui/operations/pending-operation.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import {
	OperationTarget as OperationTarget_,
	type OperationTargetOutline,
} from "#ui/routes/project/$id/workspace/OperationTarget.tsx";
import { useOperationDropTarget } from "#ui/routes/project/$id/workspace/useOperationDropTarget.ts";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { addressSpaceIncludes, type AddressSpace } from "#ui/workspace/address-space.ts";
import { mergeProps, Tooltip, useRender } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { ResizeHandle } from "#ui/components/ResizeHandle.tsx";
import uiStyles from "#ui/components/ui.module.css";
import type {
	BranchReference,
	Segment,
	Stack,
	PushStatus,
	WorktreeChanges,
	WorkspaceState,
} from "@gitbutler/but-sdk";

import { useMutationState, useQuery } from "@tanstack/react-query";
import { defaultRangeExtractor, type Range, useVirtualizer } from "@tanstack/react-virtual";
import type { PayloadFor } from "#electron/ipc.ts";
import { Match } from "effect";
import {
	type ComponentProps,
	createContext,
	type FC,
	Fragment,
	type RefObject,
	type ReactNode,
	use,
	useCallback,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import { Group, Panel, useDefaultLayout } from "react-resizable-panels";
import styles from "./WorkspaceLists.module.css";
import { Row, RowLabel, RowLabelContainer, SectionHeaderRow } from "../Row.tsx";
import { StackCard } from "../StackCard.tsx";
import stackCardStyles from "../StackCard.module.css";
import { treeItemId } from "../Row-utils.ts";
import {
	useAbsorptionTargetCommitIds,
	useAddressSpace,
	WorkspaceListsProvider,
} from "./context.tsx";
import { getOperation, type Placement, useDryRunOperation } from "#ui/operations/operation.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import { GraphSegment, type GraphSegmentStatus } from "#ui/components/GraphSegment.tsx";
import { useNow } from "#ui/components/useNow.ts";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { CommitRow } from "./CommitRow.tsx";
import { BranchRow, type PushActivity } from "./BranchRow.tsx";
import { useActiveListsHotkeys } from "./hotkeys.ts";
import { UncommittedChangesRow } from "./UncommittedChangesRow.tsx";
import { ListFilterRow } from "../ListFilterRow.tsx";
import { useListFilter } from "../useListFilter.ts";
import { buildUncommittedFileRows } from "../file-row.ts";
import { useFileDisplayMode } from "../useFileDisplayMode.ts";
import {
	canRemoveBranchReference,
	downstackPushStatusesFromSegments,
	type DownstackPushStatus,
} from "#ui/segment.ts";
import { checkedRange, addressSpaceRange } from "#ui/checking.ts";
import { TooltipPopup } from "#ui/components/Tooltip.tsx";
import { focusScope, useAutofocusScope, type FocusScope } from "#ui/focus-scopes.ts";
import { FilesTree } from "#ui/routes/project/$id/workspace/FilesTree.tsx";
import {
	CommitForm,
	type CommitTargetComboboxItem,
} from "#ui/routes/project/$id/workspace/CommitForm.tsx";
import {
	buildCommitTargetComboboxItems,
	selectCommitTargetComboboxItem,
} from "./commitTargetComboboxItems.ts";

const uncommittedChangesHeadingId = "uncommitted-changes-heading";

const DryRunWorkspaceContext = createContext<WorkspaceState | null>(null);
DryRunWorkspaceContext.displayName = "DryRunWorkspaceContext";

/** Get a virtualisation range extractor which includes the provided index, if any. */
const getRangeExtractorWithIndex = (range: Range, idx: number | undefined): Array<number> => {
	// The default range is contiguous.
	const idxs = defaultRangeExtractor(range);

	// Only indices in the virtualiser's current item set can be pinned.
	if (idx === undefined || idx < 0 || idx >= range.count) return idxs;

	const fstIdx = idxs[0];
	const lastIdx = idxs.at(-1);

	// The virtualiser positions items independently of extractor order, so an out-of-range index can
	// be safely appended in O(1). This affects DOM output order, but that's not relevant to us.
	if (fstIdx === undefined || lastIdx === undefined || idx < fstIdx || idx > lastIdx)
		idxs.push(idx);

	return idxs;
};

// This must be unique as to not collide with other IDs, and stable because it's
// stored in local storage.
type PanelId = "uncommitted-changes-panel" | "stacks-panel";

const TreeItem: FC<
	{
		address: Address;
	} & useRender.ComponentProps<"div">
> = ({ address, render, ...props }) => {
	const addressSpace = useAddressSpace();
	const isSelected = useIsCursorAt("applied", addressSpace, address);

	return useRender({
		render,
		defaultTagName: "div",
		props: mergeProps<"div">(props, {
			id: treeItemId(address),
			role: "treeitem",
			"aria-selected": isSelected,
		}),
	});
};

const OperationTarget: FC<
	{
		enabled: boolean;
		address: Address;
		projectId: string;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"button">
> = ({ enabled, address, projectId, outline, render, ...props }) => {
	const dropRef = useOperationDropTarget({ enabled, target: address, projectId });

	const absorptionTargetCommitIds = useAbsorptionTargetCommitIds();
	const addressSpace = useAddressSpace();

	type ActiveOperation = { placement: Placement; tooltip?: string | undefined };
	const selection = useSelection("applied", addressSpace);
	const activeList = useActiveList();
	const activeOperation = useAppSelector((state) => {
		const pendingOperation = projectSlice.selectors.selectPendingOperation(state, projectId);

		return Match.value(pendingOperation).pipe(
			Match.tags({
				Absorb: (): ActiveOperation | null => {
					const isActive =
						address._tag === "Commit" && absorptionTargetCommitIds.has(address.commitId);
					if (!isActive) return null;

					return { placement: "into", tooltip: "Absorb target" };
				},
				Transfer: ({ value: mode }): ActiveOperation | null => {
					if (mode.placement === null) return null;

					const target = getTransferTarget(mode, selection, activeList);
					const isActive = target !== null && addressEquals(target, address);
					if (!isActive) return null;

					return {
						placement: mode.placement,
						tooltip: getOperation({
							sources: mode.sources,
							target: address,
							placement: mode.placement,
							kind: getTransferKind(mode),
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

const AddressC: FC<
	{
		projectId: string;
		address: Address;
		outline: OperationTargetOutline;
	} & useRender.ComponentProps<"div">
> = ({ projectId, address, outline, render, ...props }) => {
	const addressSpace = useAddressSpace();

	return useRender({
		render: (
			<OperationSourceC
				projectId={projectId}
				sources={[address]}
				respectChecked={address._tag === "Commit" || address._tag === "File"}
				outline={outline}
				render={
					<OperationTarget
						enabled={addressSpaceIncludes(addressSpace, address, addressIdentityKey)}
						projectId={projectId}
						address={address}
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

const UncommittedChanges: FC<
	{
		addressSpace: AddressSpace<string>;
		commitTarget: CommitTargetComboboxItem | null;
		projectId: string;
		targetComboboxItems: Array<CommitTargetComboboxItem>;
		hasNoBranches: boolean;
		onAmendCommit: (commitId: string) => void;
		canAmendCommit: boolean;
		onActiveFileSelection: (selection: string) => void;
		onEdgeSpill: (offset: -1 | 1) => void;
		worktreeChanges: WorktreeChanges | undefined;
	} & Omit<ComponentProps<"div">, "children">
> = ({
	addressSpace,
	commitTarget,
	projectId,
	targetComboboxItems,
	hasNoBranches,
	onAmendCommit,
	canAmendCommit,
	onActiveFileSelection,
	onEdgeSpill,
	worktreeChanges,
	...props
}) => {
	const dispatch = useAppDispatch();

	const filter = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesFilter(state, projectId),
	);
	const fileDisplayMode = useFileDisplayMode();
	const recentFirst = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesRecentFirst(state, projectId),
	);
	// Ticks only while the recency view needs its labels and freshness to age.
	const ageBadgeNow = useNow(recentFirst ? 30_000 : null);
	const collapsedDirectories = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFilesCollapsedDirectories(state, projectId),
	);
	const fileRows = buildUncommittedFileRows({
		worktreeChanges,
		filter,
		mode: fileDisplayMode,
		collapsedDirectories,
		recentFirst,
	});
	const reviewedUncommittedPaths = useReviewedPaths({
		projectId,
		fileParent: uncommittedChangesFileParent,
		changes: worktreeChanges?.changes ?? [],
	});

	const fileSelection = useSelection("uncommitted", addressSpace);
	const activeList = useActiveList();

	const panelRef = useRef<HTMLDivElement>(null);
	const fileListRef = useRef<HTMLDivElement>(null);
	const fileFilter = useListFilter({
		filter,
		setFilter: (filter) =>
			dispatch(projectSlice.actions.setUncommittedFilesFilter({ projectId, filter })),
		inputId: "uncommitted-files-filter-input",
		subject: "files",
		scope: "uncommitted-files",
		selectionKey: fileSelection,
		firstKey: fileRows[0]?.path,
		onEnterList: () => {
			if (fileSelection !== null) onActiveFileSelection(fileSelection);
		},
		panelRef,
		listRef: fileListRef,
		enabled: (worktreeChanges?.changes.length ?? 0) > 0,
	});

	return (
		<div
			{...props}
			className={classes(props.className, styles.uncommittedChanges)}
			ref={useMergedRefs(props.ref, panelRef)}
		>
			{fileFilter.rowProps === null ? (
				<UncommittedChangesRow
					changes={worktreeChanges?.changes ?? []}
					headingId={uncommittedChangesHeadingId}
					projectId={projectId}
					onOpenFilter={fileFilter.open}
				/>
			) : (
				<ListFilterRow {...fileFilter.rowProps} />
			)}

			<div
				className={classes(
					uiStyles.scroller,
					uiStyles.scrollerWithSeparator,
					styles.uncommittedChangesTree,
				)}
			>
				<FilesTree
					aria-labelledby={uncommittedChangesHeadingId}
					canUncommit={false}
					data-preview-source={activeList === "uncommitted"}
					focusScope="uncommitted-files"
					emptyLabel={
						filter !== null && (worktreeChanges?.changes.length ?? 0) > 0
							? "No matching files."
							: "Nothing to commit"
					}
					fileParent={uncommittedChangesFileParent}
					reviewedPaths={reviewedUncommittedPaths}
					rows={fileRows}
					ageBadgeNow={recentFirst ? ageBadgeNow : null}
					collapsedDirectories={collapsedDirectories}
					onToggleDirectoryCollapsed={(path) =>
						dispatch(
							projectSlice.actions.toggleUncommittedFilesDirectoryCollapsed({ projectId, path }),
						)
					}
					addressSpace={addressSpace}
					onRowSelection={onActiveFileSelection}
					onEdgeSpill={onEdgeSpill}
					projectId={projectId}
					ref={useMergedRefs(fileListRef, useAutofocusScope(activeList === "uncommitted"))}
					selection={fileSelection}
				/>
			</div>

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
	stack: Stack;
	refName: BranchReference;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
	downstackPushStatus: DownstackPushStatus;
	pushActivity: PushActivity;
	isTopSegment: boolean;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	stackScrollStart: number;
	stackSize: number;
	segmentIndex: number;
	selectedCommitIndex: number | undefined;
	positionInSet: number;
	setSize: number;
}> = ({
	projectId,
	segment,
	stack,
	refName,
	canTearOffBranch,
	canRemoveBranch,
	downstackPushStatus,
	pushActivity,
	isTopSegment,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	scrollElementRef,
	stackScrollStart,
	stackSize,
	segmentIndex,
	selectedCommitIndex,
	positionInSet,
	setSize,
}) => {
	const address = branchAddress({ branchRef: refName.fullNameBytes });
	const isFolded = useAppSelector((state) =>
		projectSlice.selectors.selectSegmentFolded(
			state,
			projectId,
			decodeBytes(refName.fullNameBytes),
		),
	);

	return (
		<TreeItem
			address={address}
			aria-label={refName.displayName}
			aria-expanded={segment.commits.length > 0 ? !isFolded : undefined}
			aria-level={1}
			aria-posinset={positionInSet}
			aria-setsize={setSize}
			render={<AddressC projectId={projectId} address={address} outline="outside" />}
		>
			<BranchRow
				projectId={projectId}
				refName={refName}
				canTearOffBranch={canTearOffBranch}
				canRemoveBranch={canRemoveBranch}
				downstackPushStatus={downstackPushStatus}
				pushActivity={pushActivity}
				pushStatus={segment.pushStatus}
				recordedPullRequest={recordedPullRequest(segment)}
				graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				isTopSegment={isTopSegment}
				commitCount={segment.commits.length}
				stack={stack}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group">
				<SegmentContent
					ariaLevel={2}
					isFolded={isFolded}
					positionOffset={0}
					setSize={segment.commits.length}
					projectId={projectId}
					segment={segment}
					stackId={stack.id}
					checkCommit={checkCommit}
					onAmendCommit={onAmendCommit}
					canAmendCommit={canAmendCommit}
					scrollElementRef={scrollElementRef}
					stackScrollStart={stackScrollStart}
					stackSize={stackSize}
					segmentIndex={segmentIndex}
					selectedCommitIndex={selectedCommitIndex}
				/>
			</div>
		</TreeItem>
	);
};

const EmptySegmentContent: FC<{
	segment: Segment;
}> = ({ segment }) => {
	const addressSpace = useAddressSpace();

	const refName = assert(segment.refName);
	const inert = !addressSpaceIncludes(
		addressSpace,
		branchAddress({ branchRef: refName.fullNameBytes }),
		addressIdentityKey,
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
	stackId: string | null;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	stackScrollStart: number;
	stackSize: number;
	segmentIndex: number;
	selectedCommitIndex: number | undefined;
	ariaLevel: number;
	isFolded: boolean;
	positionOffset: number;
	setSize: number;
}> = ({
	projectId,
	segment,
	stackId,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	scrollElementRef,
	stackScrollStart,
	stackSize,
	segmentIndex,
	selectedCommitIndex,
	ariaLevel,
	isFolded,
	positionOffset,
	setSize,
}) => {
	const getCommitKey = useCallback(
		(index: number) => segment.commits[index]?.id ?? index,
		[segment.commits],
	);

	// Inline edit state lives in the selected row's DOM, so keep it mounted during manual scroll.
	const rangeExtractorWithSelected = useCallback(
		(range: Range) => getRangeExtractorWithIndex(range, selectedCommitIndex),
		[selectedCommitIndex],
	);

	const commitListRef = useRef<HTMLDivElement>(null);
	const [scrollMargin, setScrollMargin] = useState(stackScrollStart);

	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const rowVirtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: isFolded ? 0 : segment.commits.length,
		getScrollElement: () => scrollElementRef.current,
		initialOffset: () => scrollElementRef.current?.scrollTop ?? 0,
		// Keep in sync with --single-line-row-height.
		estimateSize: () => 28,
		getItemKey: getCommitKey,
		rangeExtractor: rangeExtractorWithSelected,
		scrollMargin,
		// Matches --scroll-gradient-height.
		scrollPaddingStart: 14,
		scrollPaddingEnd: 14,
	});

	const containerRef = useMergedRefs(rowVirtualizer.containerRef, commitListRef);

	// Nested commit lists share the stack scroller, so keep this segment's start in scroller
	// coordinates current when its stack or an earlier segment changes size.
	useLayoutEffect(() => {
		const element = commitListRef.current;
		if (!element) return;

		const nextScrollMargin = stackScrollStart + element.offsetTop;
		setScrollMargin((currentScrollMargin) =>
			currentScrollMargin === nextScrollMargin ? currentScrollMargin : nextScrollMargin,
		);
	}, [segmentIndex, stackScrollStart, stackSize]);

	// Activity reconnects layout effects on reveal without changing the selection. Remember the
	// handled commit so revealing the tab preserves manual scroll.
	const lastRevealedCommitIndexRef = useRef<number>(undefined);

	useLayoutEffect(() => {
		if (
			selectedCommitIndex !== undefined &&
			selectedCommitIndex !== lastRevealedCommitIndexRef.current
		)
			rowVirtualizer.scrollToIndex(selectedCommitIndex, { align: "auto" });

		lastRevealedCommitIndexRef.current = selectedCommitIndex;
	}, [rowVirtualizer, selectedCommitIndex]);

	if (segment.commits.length === 0) return <EmptySegmentContent segment={segment} />;
	// The branch row stands in for a folded segment: it takes the group glyph
	// and shows the count of the commits hidden here.
	if (isFolded) return null;

	const dryRunWorkspace = use(DryRunWorkspaceContext);
	const dryRunHeadInfoIndex = dryRunWorkspace ? getHeadInfoIndex(dryRunWorkspace.headInfo) : null;

	return (
		<div ref={containerRef} className={styles.virtualContainer}>
			{rowVirtualizer.getVirtualItems().map((virtualRow) => {
				const commit = segment.commits[virtualRow.index];
				if (commit === undefined) return null;

				const address = commitAddress({ commitId: commit.id, changeId: commit.changeId });
				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunCommitId !== undefined
						? (dryRunHeadInfoIndex?.commitContextByCommitId(dryRunCommitId)?.commit ?? null)
						: null;
				return (
					<div
						key={commit.id}
						data-index={virtualRow.index}
						ref={rowVirtualizer.measureElement}
						// We can't set fixed height here as an optimisation due to inline reword.
						style={{
							position: "absolute",
							top: 0,
							left: 0,
							width: "100%",
						}}
					>
						<TreeItem
							address={address}
							aria-label={commitTitle(commit.message) ?? "(no message)"}
							aria-level={ariaLevel}
							aria-posinset={positionOffset + virtualRow.index + 1}
							aria-setsize={setSize}
							render={
								<AddressC
									projectId={projectId}
									address={address}
									outline="outside"
									render={
										<CommitRow
											commit={commit}
											stackId={stackId}
											checkCommit={checkCommit}
											amendCommit={() => onAmendCommit(commit.id)}
											canAmendCommit={canAmendCommit}
											projectId={projectId}
											dryRunCommit={dryRunCommit}
											scrollSelectedIntoView={false}
										/>
									}
								/>
							}
						/>
					</div>
				);
			})}
		</div>
	);
};

/**
 * The rail between one segment and the next, carrying the line down past the
 * segment's last row — and, after the final segment, standing in as the card's
 * floor.
 *
 * It dims with the rows it joins, so it has to ask about the same address the
 * row above it stands for: the last commit while the segment is unfolded, and
 * the branch itself once it is folded, because folding takes the commits out of
 * the address space (see `buildAppliedAddressSpace`). Asking after a
 * folded commit would always miss, dimming the connector to half the weight of
 * the rail on either side of it and breaking the line between branches.
 */
const SegmentRailConnector: FC<{
	projectId: string;
	segment: Segment;
}> = ({ projectId, segment }) => {
	const addressSpace = useAddressSpace();

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

	const lastCommit = segment.commits.at(-1);
	const standsFor =
		lastCommit === undefined || isFolded
			? branchAddress({ branchRef: assert(segment.refName).fullNameBytes })
			: commitAddress({ commitId: lastCommit.id, changeId: lastCommit.changeId });

	return (
		<Row
			interactive={false}
			className={stackCardStyles.railConnector}
			inert={!addressSpaceIncludes(addressSpace, standsFor, addressIdentityKey)}
		>
			<GraphSegment
				glyph="parent"
				status={
					lastCommit === undefined
						? segmentPushStatusToGraphSegmentStatus(segment.pushStatus)
						: commitIsDiverged(lastCommit)
							? "Diverged"
							: lastCommit.state.type
				}
			/>
		</Row>
	);
};

const StackC: FC<
	{
		projectId: string;
		stack: Stack;
		checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
		onAmendCommit: (commitId: string) => void;
		canAmendCommit: boolean;
		pendingPushBranches: Set<string>;
		scrollElementRef: RefObject<HTMLDivElement | null>;
		stackScrollStart: number;
		stackSize: number;
		selectedSegmentIndex: number | undefined;
		selectedCommitIndex: number | undefined;
	} & ComponentProps<"div">
> = ({
	projectId,
	stack,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	pendingPushBranches,
	scrollElementRef,
	stackScrollStart,
	stackSize,
	selectedSegmentIndex,
	selectedCommitIndex,
	...props
}) => {
	const canTearOffBranch = stack.segments.length > 1;
	const downstackPushStatuses = downstackPushStatusesFromSegments(stack.segments);
	const topmostPendingPushIndex = stack.segments.findIndex(
		(segment) =>
			segment.refName && pendingPushBranches.has(decodeBytes(segment.refName.fullNameBytes)),
	);
	// Each stack group is a root sibling set. A branch is one root item whose commits are children;
	// an unbranched segment contributes its commits directly to the root set.
	const rootPositionOffsets: Array<number> = [];
	let rootSetSize = 0;
	for (const segment of stack.segments) {
		rootPositionOffsets.push(rootSetSize);
		rootSetSize += segment.refName === null ? segment.commits.length : 1;
	}

	return (
		<StackCard
			{...props}
			className={classes(props.className, styles.virtualStack)}
			// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- This is a group of treeitems.
			role="group"
			aria-label="Stack"
		>
			{stack.segments.map((segment, index) => {
				// oxlint-disable-next-line typescript/no-non-null-assertion -- Equivalent iteration above.
				const segmentPositionOffset = rootPositionOffsets[index]!;

				const key = segment.refName
					? JSON.stringify(segment.refName.fullNameBytes)
					: segment.commits[0]?.id;

				// A segment is supposed to always either have a branch reference or at least one commit,
				// however with the current API this may not be the case e.g. detached HEAD.
				if (key === undefined) return null;

				const downstackPushStatus = assert(downstackPushStatuses[index]);
				const pushActivity: PushActivity =
					topmostPendingPushIndex !== -1
						? index >= topmostPendingPushIndex
							? "pushing"
							: "blocked"
						: "idle";

				return (
					<Fragment key={key}>
						<div>
							{segment.refName ? (
								<BranchSegment
									projectId={projectId}
									segment={segment}
									stack={stack}
									refName={segment.refName}
									canTearOffBranch={canTearOffBranch}
									canRemoveBranch={canRemoveBranchReference(stack, index)}
									downstackPushStatus={downstackPushStatus}
									pushActivity={pushActivity}
									isTopSegment={index === 0}
									checkCommit={checkCommit}
									onAmendCommit={onAmendCommit}
									canAmendCommit={canAmendCommit}
									scrollElementRef={scrollElementRef}
									stackScrollStart={stackScrollStart}
									stackSize={stackSize}
									segmentIndex={index}
									positionInSet={segmentPositionOffset + 1}
									setSize={rootSetSize}
									selectedCommitIndex={
										selectedSegmentIndex === index ? selectedCommitIndex : undefined
									}
								/>
							) : (
								<SegmentContent
									ariaLevel={1}
									isFolded={false}
									positionOffset={segmentPositionOffset}
									setSize={rootSetSize}
									projectId={projectId}
									segment={segment}
									stackId={stack.id}
									checkCommit={checkCommit}
									onAmendCommit={onAmendCommit}
									canAmendCommit={canAmendCommit}
									scrollElementRef={scrollElementRef}
									stackScrollStart={stackScrollStart}
									stackSize={stackSize}
									segmentIndex={index}
									selectedCommitIndex={
										selectedSegmentIndex === index ? selectedCommitIndex : undefined
									}
								/>
							)}
						</div>
						<SegmentRailConnector projectId={projectId} segment={segment} />
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
	const addressSpace = useAddressSpace();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selection = useSelection("applied", addressSpace);
	const activeList = useActiveList();
	const dryRunOperation = useAppSelector((state) => {
		const pendingOperation = projectSlice.selectors.selectPendingOperation(state, projectId);

		return Match.value(pendingOperation).pipe(
			Match.tags({
				Transfer: ({ value: mode }) => {
					if (mode.placement === null) return;

					const target = getTransferTarget(mode, selection, activeList);
					if (!target) return;

					return getOperation({
						sources: mode.sources,
						target,
						placement: mode.placement,
						kind: getTransferKind(mode),
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
	const stacks = (headInfo?.stacks ?? []).toReversed();
	const foldedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectFoldedSegments(state, projectId),
	);
	const pendingPushBranches = new Set(
		useMutationState({
			filters: {
				mutationKey: [projectId, "workspaceBranchAndAncestorsPush"],
				status: "pending",
			},
			select: (mutation) =>
				(mutation.state.variables as PayloadFor<"workspaceBranchAndAncestorsPush">).branch,
		}),
	);
	const scrollElementRef = useRef<HTMLDivElement>(null);
	const retainScrollElement = useCallback((element: HTMLDivElement | null) => {
		if (element) scrollElementRef.current = element;
	}, []);
	const getStackKey = useCallback((index: number) => stacks[index]?.id ?? index, [stacks]);
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
	const selectedContext =
		selection?._tag === "Branch"
			? headInfoIndex?.branchContextByRefBytes(selection.branchRef)
			: selection?._tag === "Commit"
				? headInfoIndex?.commitContextByCommitId(selection.commitId)
				: undefined;
	const selectedStackIndex =
		selectedContext === undefined ? undefined : stacks.length - selectedContext.stackIndex - 1;
	const selectedSegmentIndex = selectedContext?.segmentIndex;
	const selectedCommitIndex =
		selection?._tag === "Commit"
			? headInfoIndex?.commitContextByCommitId(selection.commitId)?.commitIndex
			: undefined;

	// Pin the containing stack too, otherwise the nested selected row can still be unmounted.
	const rangeExtractorWithSelected = useCallback(
		(range: Range) => getRangeExtractorWithIndex(range, selectedStackIndex),
		[selectedStackIndex],
	);

	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const rowVirtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: stacks.length,
		getScrollElement: () => scrollElementRef.current,
		estimateSize: (index) => {
			// Keep in sync with Row.module.css and StackCard.module.css. Measurements replace this
			// estimate once a stack is mounted; its main job is to make far-away stacks reachable.
			const singleLineRowHeight = 28;
			const branchRowHeight = 54;
			const stackBodyPaddingStart = 6;
			const stackBorderHeight = 1;
			const stackSeparatorHeight = index === 0 ? 0 : 1;
			const finalConnectorHeight = 8;
			const betweenSegmentConnectorHeight = 14;
			const stack = stacks[index];
			if (stack === undefined) return singleLineRowHeight;

			let contentHeight = 0;
			for (const segment of stack.segments) {
				if (segment.refName !== null) contentHeight += branchRowHeight;

				const isFolded =
					segment.refName !== null &&
					foldedSegments[decodeBytes(segment.refName.fullNameBytes)] === true;
				if (!isFolded) contentHeight += Math.max(1, segment.commits.length) * singleLineRowHeight;
			}

			return (
				stackBodyPaddingStart +
				stackBorderHeight +
				stackSeparatorHeight +
				contentHeight +
				finalConnectorHeight +
				Math.max(0, stack.segments.length - 1) * betweenSegmentConnectorHeight
			);
		},
		getItemKey: getStackKey,
		rangeExtractor: rangeExtractorWithSelected,
		gap: 10,
		// Matches --scroll-gradient-height.
		scrollPaddingStart: 14,
		scrollPaddingEnd: 14,
	});

	const selectedAddressKey = selection === null ? undefined : addressIdentityKey(selection);
	const lastRevealedAddressKeyRef = useRef<string>(undefined);

	// If the selected row's stack is not rendered, reveal the stack first. Its branch row or nested
	// commit virtualizer then handles precise alignment.
	useLayoutEffect(() => {
		const selectedStackIsMounted = rowVirtualizer
			.getVirtualItems()
			.some((virtualRow) => virtualRow.index === selectedStackIndex);

		if (
			selectedAddressKey !== undefined &&
			selectedAddressKey !== lastRevealedAddressKeyRef.current &&
			selectedStackIndex !== undefined &&
			!selectedStackIsMounted
		)
			rowVirtualizer.scrollToIndex(selectedStackIndex, { align: "auto" });

		lastRevealedAddressKeyRef.current = selectedAddressKey;
	}, [rowVirtualizer, selectedAddressKey, selectedStackIndex]);

	const hotkeysRef = useRef<HTMLDivElement>(null);
	useActiveListsHotkeys({
		addressSpace,
		projectId,
		ref: hotkeysRef,
		checkCommit,
		focusCommitMessageInput,
		onEdgeSpill,
		pendingPushBranches,
	});

	return (
		<DryRunWorkspaceContext value={dryRunWorkspace}>
			<div ref={retainScrollElement} className={classes(uiStyles.scroller, styles.stacksScroller)}>
				<div
					tabIndex={0}
					role="tree"
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					className={classes(styles.tree, styles.stacks, styles.virtualContainer)}
					data-focus-scope={"sidebar" satisfies FocusScope}
					data-preview-source={activeList === "applied"}
					ref={useMergedRefs(
						rowVirtualizer.containerRef,
						hotkeysRef,
						useAutofocusScope(activeList === "applied"),
					)}
				>
					{rowVirtualizer.getVirtualItems().map((virtualRow) => {
						const stack = stacks[virtualRow.index];
						if (stack === undefined) return null;

						return (
							<StackC
								key={stack.id ?? virtualRow.index}
								data-index={virtualRow.index}
								ref={rowVirtualizer.measureElement}
								style={{
									position: "absolute",
									top: 0,
									left: 0,
									width: "100%",
									transform: `translateY(${virtualRow.start}px)`,
								}}
								projectId={projectId}
								stack={stack}
								checkCommit={checkCommit}
								onAmendCommit={onAmendCommit}
								canAmendCommit={canAmendCommit}
								pendingPushBranches={pendingPushBranches}
								scrollElementRef={scrollElementRef}
								stackScrollStart={virtualRow.start}
								stackSize={virtualRow.size}
								selectedSegmentIndex={
									selectedStackIndex === virtualRow.index ? selectedSegmentIndex : undefined
								}
								selectedCommitIndex={
									selectedStackIndex === virtualRow.index ? selectedCommitIndex : undefined
								}
							/>
						);
					})}
				</div>
			</div>
		</DryRunWorkspaceContext>
	);
};

export const WorkspaceLists: FC<
	{
		projectId: string;
		addressSpace: AddressSpace<Address>;
		uncommittedAddressSpace: AddressSpace<string>;
		absorptionTargetCommitIds: ReadonlySet<string>;
		onActiveFileSelection: (selection: string) => void;
		stacksHeaderActions?: ReactNode;
	} & ComponentProps<"div">
> = ({
	projectId,
	addressSpace,
	uncommittedAddressSpace,
	absorptionTargetCommitIds,
	onActiveFileSelection,
	stacksHeaderActions,
	...props
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;

	const appliedSelection = useSelection("applied", addressSpace);
	const commitTargetComboboxItems = buildCommitTargetComboboxItems({
		headInfo,
		headInfoIndex,
		appliedSelection,
	});
	const commitTarget = selectCommitTargetComboboxItem({
		items: commitTargetComboboxItems,
		appliedSelection,
	});
	// Undefined `headInfo` is still loading, which is not the same as "empty" —
	// treating it as empty would flash the draft-branch affordance on every open.
	const hasNoBranches = headInfo !== undefined && headInfo.stacks.length === 0;
	const store = useAppStore();
	const dispatch = useAppDispatch();
	const { isPending: isCommitAmendPending, mutate: commitAmend } = useCommitAmend(projectId);
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
			changes: worktreeChanges.changes
				.values()
				.filter(
					(change) =>
						checkedUncommittedFilePaths.size === 0 || checkedUncommittedFilePaths.has(change.path),
				)
				.map((change) => createDiffSpec(change, []))
				.toArray(),
			changesSource: { type: "head" },
			dryRun: false,
		});
	};

	const commitCheckRangeAnchor = useRef<string>(null);
	const commitCheckRangeEnd = useRef<string>(null);

	const rangeResolver = addressSpaceRange<Address, string>({
		addressSpace,
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
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: checkedCommits
					.values()
					.map((commitId) => {
						const ctx = headInfoIndex?.commitContextByCommitId(commitId);
						return ctx ? commitAddress({ commitId, changeId: ctx.commit.changeId }) : null;
					})
					.filter((x) => x != null)
					.toArray(),
				checked: true,
			}),
		);
		dispatch(
			projectSlice.actions.checkAddresses({
				projectId,
				addresses: uncheckedCommits
					.values()
					.map((commitId) => {
						const ctx = headInfoIndex?.commitContextByCommitId(commitId);
						return ctx ? commitAddress({ commitId, changeId: ctx.commit.changeId }) : null;
					})
					.filter((x) => x != null)
					.toArray(),
				checked: false,
			}),
		);
	};

	const layoutId = `project=${projectId}:sidebar-tree`;
	const sidebarLayout = useDefaultLayout({
		id: layoutId,
		panelIds: ["uncommitted-changes-panel", "stacks-panel"] satisfies Array<PanelId>,
	});

	// The two panes stack vertically, so arrow navigation continues across
	// their boundary: entering a pane selects its item nearest to the border,
	// while the pane being left keeps its selection. An empty neighbor keeps
	// focus where it is. Mod+Alt+arrow pane toggling stays selection-neutral.
	const spillIntoStacks = (offset: -1 | 1) => {
		if (offset !== 1) return;
		const item = addressSpace.items.at(0);
		if (item === undefined) return;
		setCursor("applied", item);
		focusScope("sidebar");
	};
	const spillIntoUncommittedChanges = (offset: -1 | 1) => {
		if (offset !== -1) return;
		const path = uncommittedAddressSpace.items.at(-1);
		if (path === undefined) return;
		onActiveFileSelection(path);
		focusScope("uncommitted-files");
	};

	return (
		<WorkspaceListsProvider
			addressSpace={addressSpace}
			absorptionTargetCommitIds={absorptionTargetCommitIds}
		>
			<Group
				{...props}
				id={layoutId}
				orientation="vertical"
				className={classes(props.className, styles.tree)}
				defaultLayout={sidebarLayout.defaultLayout}
				onLayoutChanged={sidebarLayout.onLayoutChanged}
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
						sources={[uncommittedChangesAddress]}
						respectChecked={false}
						outline="inside"
						render={
							<OperationTarget
								enabled
								projectId={projectId}
								address={uncommittedChangesAddress}
								outline="inside"
								render={
									<UncommittedChanges
										addressSpace={uncommittedAddressSpace}
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
						className={styles.stacksHeader}
						actions={stacksHeaderActions}
					/>

					<Stacks
						projectId={projectId}
						checkCommit={checkCommit}
						onAmendCommit={amendCommit}
						canAmendCommit={canAmendCommit}
						onEdgeSpill={spillIntoUncommittedChanges}
					/>
				</Panel>
			</Group>
		</WorkspaceListsProvider>
	);
};
