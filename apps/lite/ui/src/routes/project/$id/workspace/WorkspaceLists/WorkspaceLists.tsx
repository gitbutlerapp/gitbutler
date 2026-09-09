import rowStyles from "../Row.module.css";
import { setCursor, useActiveList, useSelection } from "#ui/use-cursor.ts";
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
	commitIdentityKey,
} from "#ui/addresses.ts";
import { useReviewedPaths } from "#ui/routes/project/$id/workspace/reviewed-paths.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { getTransferKind, getTransferTarget } from "#ui/operations/pending-operation.ts";
import { OperationSourceC } from "#ui/routes/project/$id/workspace/OperationSourceC.tsx";
import { AddressC, OperationTarget, TreeItem } from "./TreeItem.tsx";
import { WorktreeCard, WorktreeLane, WorktreeOnTip } from "./WorktreeLane.tsx";
import { useAppDispatch, useAppSelector, useAppStore } from "#ui/store.ts";
import { classes } from "#ui/components/classes.ts";
import { addressSpaceIncludes, type AddressSpace } from "#ui/workspace/address-space.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import uiStyles from "#ui/components/ui.module.css";
import type {
	BranchReference,
	Commit,
	Segment,
	Stack,
	PushStatus,
	WorktreeChanges,
	WorkspaceState,
	Worktree,
} from "@gitbutler/but-sdk";

import { useMutationState, useQuery } from "@tanstack/react-query";
import { type Range, useVirtualizer } from "@tanstack/react-virtual";
import type { PayloadFor } from "#electron/ipc.ts";
import { Match } from "effect";
import {
	Activity,
	type CSSProperties,
	Fragment,
	createContext,
	use,
	useCallback,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	type ComponentProps,
	type FC,
	type ReactNode,
	type RefObject,
} from "react";
import styles from "./WorkspaceLists.module.css";
import { Row, RowLabel, RowLabelContainer, SectionHeaderRow } from "../Row.tsx";
import { Section } from "../Graph/Section.tsx";
import {
	CARD_GAP,
	DOCKED_HEIGHT,
	HEAD_DOCKED_HEIGHT,
	ROW_INSET,
	inSection,
	sectionAddresses,
	type WorktreePlacement,
	worktreesOnTip,
} from "../Graph/layout.ts";
import type { Graph } from "../Graph/usePlan.ts";
import { StackCard } from "../StackCard.tsx";
import stackCardStyles from "../StackCard.module.css";
import { treeItemId } from "../Row-utils.ts";
import { useAddressSpace, WorkspaceListsProvider } from "./context.tsx";
import { getOperation, useDryRunOperation } from "#ui/operations/operation.ts";
import { createDiffSpec } from "#ui/operations/diff-specs.ts";
import {
	GraphEdge,
	GraphGap,
	GraphSegment,
	type GraphSegmentStatus,
} from "#ui/components/GraphSegment.tsx";
import { useNow } from "#ui/components/useNow.ts";
import { segmentBottomRelativeTo } from "#ui/api/stack.ts";
import { assert } from "#ui/assert.ts";
import { CommitRow } from "./CommitRow.tsx";
import { IncomingRows } from "./IncomingRows.tsx";
import { BranchRow, type PushActivity } from "./BranchRow.tsx";
import { useActiveListsHotkeys } from "./hotkeys.ts";
import { UncommittedChangesRow } from "./UncommittedChangesRow.tsx";
import { LastCommitLine } from "./LastCommitLine.tsx";
import { NoStacks } from "./NoStacks.tsx";
import type { NewBranchActions } from "../useNewBranch.ts";
import { ListFilterRow } from "../ListFilterRow.tsx";
import { useListFilter } from "../useListFilter.ts";
import { buildUncommittedFileRows } from "../file-row.ts";
import { useFileDisplayMode } from "../useFileDisplayMode.ts";
import {
	canIntegrateUpstream,
	canRemoveBranchReference,
	downstackPushStatusesFromSegments,
	type DownstackPushStatus,
} from "#ui/segment.ts";
import { checkedRange, addressSpaceRange } from "#ui/checking.ts";
import { focusScope, useAutofocusScope, type FocusScope } from "#ui/focus-scopes.ts";
import { getRangeExtractorWithIndices } from "#ui/virtual.ts";
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

/**
 * An element's height, kept current as it resizes: give the element the ref.
 * Keyed on the element, not a ref object, so a replaced node (a hot reload
 * swaps them) is measured afresh rather than watched after it is gone.
 */
const noLanes: ReadonlyArray<Worktree> = [];

const useHeight = (): [ref: (element: HTMLElement | null) => void, height: number] => {
	const [element, setElement] = useState<HTMLElement | null>(null);
	const [height, setHeight] = useState(0);
	useLayoutEffect(() => {
		if (element === null) return;
		const measure = () => setHeight(element.offsetHeight);
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(element);
		return () => observer.disconnect();
	}, [element]);
	return [setElement, height];
};

const UncommittedChanges: FC<
	{
		addressSpace: AddressSpace<string>;
		commitTarget: CommitTargetComboboxItem | null;
		projectId: string;
		targetComboboxItems: Array<CommitTargetComboboxItem>;
		hasNoBranches: boolean;
		/**
		 * Not `on*`-named on purpose, nor are the two callbacks below: this
		 * component is the `render` element of an operation target, and base-ui's
		 * `mergeProps` wraps every `on*` function prop it passes through in a new
		 * function. Callbacks that arrive wrapped change identity on every render,
		 * and everything below that keys on them re-renders with them.
		 */
		amendCommit: (commitId: string) => void;
		canAmendCommit: boolean;
		selectActiveFile: (selection: string) => void;
		spillEdge: (offset: -1 | 1) => void;
		worktreeChanges: WorktreeChanges | undefined;
		/** The graph's scroller, which the card heads. */
		scrollElementRef: RefObject<HTMLDivElement | null>;
		/** The docked target row's height at the scroller's foot, or 0: the commit form sticks above it. */
		footDock: number;
		/** The card's head, measured by the parent, which docks a stand-in as soon as the head is pushed. */
		headRef: (element: HTMLElement | null) => void;
		headHeight: number;
	} & Omit<ComponentProps<"div">, "children">
> = ({
	addressSpace,
	commitTarget,
	projectId,
	targetComboboxItems,
	hasNoBranches,
	amendCommit,
	canAmendCommit,
	selectActiveFile,
	spillEdge,
	worktreeChanges,
	scrollElementRef,
	footDock,
	headRef,
	headHeight,
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
	const folded = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFolded(state, projectId),
	);
	const toggleFolded = () => dispatch(projectSlice.actions.toggleUncommittedFolded({ projectId }));
	// Loaded and holding nothing, as opposed to not loaded yet: the header takes
	// over the empty wording, so it must not say it before the answer is in.
	const isClean = worktreeChanges !== undefined && worktreeChanges.changes.length === 0;

	const cardRef = useRef<HTMLDivElement>(null);
	const fileListRef = useRef<HTMLDivElement>(null);
	// The head sticks at the scroller's top and the commit form at its foot, so a row
	// scrolled into view clears both.
	const [formRef, formHeight] = useHeight();
	// The list's start in the scroller, which the card heads: the rows above the
	// list come and go with the filter and the worktree, so the card's size says
	// when to measure again.
	const [listOffset, setListOffset] = useState(0);
	useLayoutEffect(() => {
		const card = cardRef.current;
		if (card === null) return;
		const measure = () => {
			const list = fileListRef.current;
			if (list === null) return;
			setListOffset(
				Math.max(0, list.getBoundingClientRect().top - card.getBoundingClientRect().top),
			);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(card);
		return () => observer.disconnect();
	}, []);
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
			if (fileSelection !== null) selectActiveFile(fileSelection);
		},
		panelRef: cardRef,
		listRef: fileListRef,
		enabled: !folded && (worktreeChanges?.changes.length ?? 0) > 0,
	});
	// The trunk runs down the card as its own line, the rows' rail.
	const trunk = <GraphEdge glyph="parent" />;

	return (
		<div
			{...props}
			className={classes(props.className, styles.uncommittedCard)}
			ref={useMergedRefs(props.ref, cardRef)}
		>
			<div ref={headRef} className={styles.cardHead}>
				<div className={styles.cardHeadContent}>
					<div className={styles.pad} />
					<UncommittedChangesRow
						changes={worktreeChanges?.changes ?? []}
						isClean={isClean}
						projectId={projectId}
						mode={{
							kind: "card",
							headingId: uncommittedChangesHeadingId,
							folded,
							onToggleFolded: toggleFolded,
							onOpenFilter: fileFilter.open,
						}}
					/>
					<Activity mode={folded ? "hidden" : "visible"}>
						{fileFilter.rowProps !== null && (
							<ListFilterRow {...fileFilter.rowProps} rail={trunk} />
						)}
					</Activity>
				</div>
			</div>

			{/* Folded, the header stands for the card: its file count and line stats
			    are the only sign left that there is uncommitted work. Hidden rather
			    than unmounted, as with the sidebar's own pages, so the list comes back
			    scrolled and filtered the way it was left. */}
			<Activity mode={folded ? "hidden" : "visible"}>
				{isClean && (
					<Row interactive={false}>
						{trunk}
						<RowLabelContainer>
							<LastCommitLine projectId={projectId} />
						</RowLabelContainer>
					</Row>
				)}

				{/* A clean worktree drops the list as well: the header says so now, and
				    an empty row under it would only say it twice. An unloaded one drops
				    it too — its rows are empty for want of an answer, not because there
				    is none, and the empty row would otherwise flash "Nothing to commit"
				    under a header still reading "Uncommitted". */}
				<Activity mode={isClean || worktreeChanges === undefined ? "hidden" : "visible"}>
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
								projectSlice.actions.toggleUncommittedFilesDirectoryCollapsed({
									projectId,
									path,
								}),
							)
						}
						addressSpace={addressSpace}
						onRowSelection={selectActiveFile}
						onEdgeSpill={spillEdge}
						projectId={projectId}
						ref={useMergedRefs(fileListRef, useAutofocusScope(activeList === "uncommitted"))}
						selection={fileSelection}
						rail={trunk}
						scrollElementRef={scrollElementRef}
						scrollMargin={listOffset}
						scrollPaddingStart={headHeight}
						scrollPaddingEnd={footDock + formHeight}
						// The rows sit on the trunk, whose edge column is their whole gutter: no inset, the tree's own included.
						style={{ "--row-padding-inline-start": "0px" }}
					/>
				</Activity>

				<div ref={formRef} className={styles.commitFoot} style={{ bottom: footDock }}>
					<Row interactive={false} className={styles.commitFootRow}>
						{trunk}
						<CommitForm
							projectId={projectId}
							commitTarget={commitTarget}
							targetComboboxItems={targetComboboxItems}
							hasNoBranches={hasNoBranches}
							startCommitButtonId={startCommitButtonId}
							commitMessageInputId={commitMessageInputId}
							className={styles.commitForm}
							onAmendCommit={amendCommit}
							canAmendCommit={canAmendCommit}
							worktreeChanges={worktreeChanges}
						/>
					</Row>
				</div>
			</Activity>

			<Row interactive={false} className={styles.stub}>
				{trunk}
			</Row>
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

/** A commit's glyph colour: its state's, or the diverged one's. */
const commitGraphStatus = (commit: Commit): GraphSegmentStatus =>
	commitIsDiverged(commit) ? "Diverged" : commit.state.type;

const BranchSegment: FC<{
	projectId: string;
	segment: Segment;
	stack: Stack;
	refName: BranchReference;
	canTearOffBranch: boolean;
	canRemoveBranch: boolean;
	downstackPushStatus: DownstackPushStatus;
	pushActivity: PushActivity;
	startsRail: boolean;
	behind: number;
	worktrees: WorktreePlacement;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	scrollPaddingEnd: number;
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
	startsRail,
	behind,
	worktrees,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	scrollElementRef,
	scrollPaddingEnd,
	stackScrollStart,
	stackSize,
	segmentIndex,
	selectedCommitIndex,
	positionInSet,
	setSize,
}) => {
	const address = branchAddress({ branchRef: refName.fullNameBytes });
	// The rail below the branch's tick is its first commit's; plain when it has none.
	const firstCommit = segment.commits[0];
	const railBelow = firstCommit === undefined ? "LocalOnly" : commitGraphStatus(firstCommit);
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
				canUpdateFromRemote={canIntegrateUpstream(segment)}
				remote={segment.remoteTrackingRefName}
				incoming={segment.commitsOnRemote.length}
				recordedPullRequest={recordedPullRequest(segment)}
				graphStatus={segmentPushStatusToGraphSegmentStatus(segment.pushStatus)}
				bottomRelativeTo={segmentBottomRelativeTo(segment)}
				startsRail={startsRail}
				commitCount={segment.commits.length}
				railBelow={railBelow}
				behind={behind}
				stack={stack}
			/>

			{/* oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics. */}
			<div role="group">
				{/* Gated here so the common case pays no mount; folding hides them with the commits. */}
				{!isFolded && segment.commitsOnRemote.length > 0 && (
					<IncomingRows projectId={projectId} segment={segment} refName={refName} behind={behind} />
				)}
				<SegmentContent
					ariaLevel={2}
					isFolded={isFolded}
					positionOffset={0}
					setSize={segment.commits.length}
					projectId={projectId}
					segment={segment}
					stackId={stack.id}
					behind={behind}
					worktrees={worktrees}
					checkCommit={checkCommit}
					onAmendCommit={onAmendCommit}
					canAmendCommit={canAmendCommit}
					scrollElementRef={scrollElementRef}
					scrollPaddingEnd={scrollPaddingEnd}
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
	behind: number;
}> = ({ segment, behind }) => {
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
					behind={behind}
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
	scrollPaddingEnd: number;
	stackScrollStart: number;
	stackSize: number;
	segmentIndex: number;
	selectedCommitIndex: number | undefined;
	ariaLevel: number;
	isFolded: boolean;
	positionOffset: number;
	setSize: number;
	behind: number;
	worktrees: WorktreePlacement;
}> = ({
	projectId,
	segment,
	stackId,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	scrollElementRef,
	scrollPaddingEnd,
	stackScrollStart,
	stackSize,
	segmentIndex,
	selectedCommitIndex,
	ariaLevel,
	isFolded,
	positionOffset,
	setSize,
	behind,
	worktrees,
}) => {
	const getCommitKey = useCallback(
		(index: number) => segment.commits[index]?.id ?? index,
		[segment.commits],
	);

	// Inline edit state lives in the selected row's DOM, so keep it mounted during manual scroll.
	const rangeExtractorWithSelected = useCallback(
		(range: Range) =>
			getRangeExtractorWithIndices(
				range,
				selectedCommitIndex === undefined ? [] : [selectedCommitIndex],
			),
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
		// Matches --scroll-gradient-height; the foot also clears the docked target row.
		scrollPaddingStart: 14,
		scrollPaddingEnd,
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

	if (segment.commits.length === 0)
		return <EmptySegmentContent segment={segment} behind={behind} />;
	// The lanes drawn above a commit row. The worktrees on the top branch's tip
	// are drawn above that branch by the card instead, see `worktreesOnTip`.
	const lanesOn = (commit: Commit, index: number): ReadonlyArray<Worktree> =>
		segmentIndex === 0 && index === 0 && segment.refName !== null
			? noLanes
			: (worktrees.on.get(commit.id) ?? noLanes);

	// The branch row stands in for a folded segment: it takes the group glyph
	// and shows the count of the commits hidden here. The worktrees resting on
	// those commits stay, or folding a branch would make a worktree vanish.
	if (isFolded) {
		return segment.commits
			.flatMap((commit, index) => lanesOn(commit, index))
			.map((worktree) => (
				<WorktreeLane
					key={worktree.name}
					projectId={projectId}
					worktree={worktree}
					worktrees={worktrees}
					behind={behind + 1}
				/>
			));
	}

	const dryRunWorkspace = use(DryRunWorkspaceContext);
	const dryRunHeadInfoIndex = dryRunWorkspace ? getHeadInfoIndex(dryRunWorkspace.headInfo) : null;

	return (
		<div ref={containerRef} className={styles.virtualContainer}>
			{rowVirtualizer.getVirtualItems().map((virtualRow) => {
				const commit = segment.commits[virtualRow.index];
				if (commit === undefined) return null;
				// The rail below the commit is the next one's; plain under the last.
				const next = segment.commits[virtualRow.index + 1];

				const dryRunCommitId = dryRunWorkspace?.replacedCommits[commit.id];
				const dryRunCommit =
					dryRunCommitId !== undefined
						? (dryRunHeadInfoIndex?.commitContextByCommitId(dryRunCommitId)?.commit ?? null)
						: null;
				return (
					<CommitItem
						key={commit.id}
						index={virtualRow.index}
						measureElement={rowVirtualizer.measureElement}
						commit={commit}
						below={next === undefined ? "LocalOnly" : commitGraphStatus(next)}
						behind={behind}
						lanes={lanesOn(commit, virtualRow.index)}
						worktrees={worktrees}
						projectId={projectId}
						stackId={stackId}
						checkCommit={checkCommit}
						onAmendCommit={onAmendCommit}
						canAmendCommit={canAmendCommit}
						dryRunCommit={dryRunCommit}
						ariaLevel={ariaLevel}
						positionInSet={positionOffset + virtualRow.index + 1}
						setSize={setSize}
					/>
				);
			})}
		</div>
	);
};

/**
 * One virtualised commit row. Its own component so that a re-render of the
 * segment leaves the row's element tree cached when nothing about the commit
 * changed: {@link SegmentContent} is left uncompiled by its virtualizer, so
 * anything built inline there is rebuilt, and re-renders every row, on every
 * render.
 */
const CommitItem: FC<{
	index: number;
	measureElement: (element: HTMLDivElement | null) => void;
	commit: Commit;
	below: GraphSegmentStatus;
	behind: number;
	/** The worktree lanes drawn above this commit, resting on it. */
	lanes: ReadonlyArray<Worktree>;
	worktrees: WorktreePlacement;
	projectId: string;
	stackId: string | null;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	dryRunCommit: Commit | null;
	ariaLevel: number;
	positionInSet: number;
	setSize: number;
}> = ({
	index,
	measureElement,
	commit,
	below,
	behind,
	lanes,
	worktrees,
	projectId,
	stackId,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	dryRunCommit,
	ariaLevel,
	positionInSet,
	setSize,
}) => {
	const address = commitAddress({ commitId: commit.id, changeId: commit.changeId });

	return (
		<div
			data-index={index}
			ref={measureElement}
			// We can't set fixed height here as an optimisation due to inline reword.
			style={{
				position: "absolute",
				top: 0,
				left: 0,
				width: "100%",
			}}
		>
			{/* Inside the measured element: the virtualizer sizes the commit's item, lanes included. */}
			{lanes.map((worktree) => (
				<WorktreeLane
					key={worktree.name}
					projectId={projectId}
					worktree={worktree}
					worktrees={worktrees}
					behind={behind + 1}
				/>
			))}
			<TreeItem
				address={address}
				aria-label={commitTitle(commit.message) ?? "(no message)"}
				aria-level={ariaLevel}
				aria-posinset={positionInSet}
				aria-setsize={setSize}
				render={
					<AddressC
						projectId={projectId}
						address={address}
						outline="outside"
						render={
							<CommitRow
								commit={commit}
								below={below}
								behind={behind}
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
	behind: number;
}> = ({ projectId, segment, behind }) => {
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
			{/* Plain: a branch's colour runs from its tick down to its commits, not past them. */}
			<GraphSegment glyph="parent" status="LocalOnly" behind={behind} />
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
		scrollPaddingEnd: number;
		stackScrollStart: number;
		stackSize: number;
		/** The list's start in the scroller, which the card's own position is from. */
		scrollMargin: number;
		worktrees: WorktreePlacement;
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
	scrollPaddingEnd,
	stackScrollStart,
	stackSize,
	scrollMargin,
	worktrees,
	selectedSegmentIndex,
	selectedCommitIndex,
	...props
}) => {
	const canTearOffBranch = stack.segments.length > 1;
	// Manual memo: the compiler folds this into the props scope, where it is rebuilt
	// on every render and hands each branch row a fresh status.
	const downstackPushStatuses = useMemo(
		() => downstackPushStatusesFromSegments(stack.segments),
		[stack.segments],
	);
	// Built here rather than passed in: the uncompiled parent would rebuild the object,
	// and with it this whole card, on every render.
	const style: CSSProperties = {
		position: "absolute",
		top: 0,
		left: 0,
		width: "100%",
		transform: `translateY(${stackScrollStart - scrollMargin}px)`,
	};
	// A card is a lane off the trunk, which runs behind it at the edge.
	const behind = 1;
	const topmostPendingPushIndex = stack.segments.findIndex(
		(segment) =>
			segment.refName && pendingPushBranches.has(decodeBytes(segment.refName.fullNameBytes)),
	);
	// Worktrees on the top branch's tip continue the card's line above it, so the
	// branch row joins a rail that starts at the worktree rather than starting one.
	const onTip = worktreesOnTip(worktrees, stack);
	// Each stack group is a root sibling set. A branch is one root item whose commits are children;
	// an unbranched segment contributes its commits directly to the root set.
	const rootPositionOffsets: Array<number> = [];
	let rootSetSize = 0;
	for (const segment of stack.segments) {
		rootPositionOffsets.push(rootSetSize);
		rootSetSize += segment.refName === null ? segment.commits.length : 1;
	}

	return (
		<div {...props} style={style} className={classes(props.className, styles.stack)}>
			<StackCard
				className={styles.virtualStack}
				// In the graph the card draws its rails to its edges: see StackCard.module.css.
				data-graph
				// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- This is a group of treeitems.
				role="group"
				aria-label="Stack"
			>
				<Row interactive={false} className={styles.pad}>
					<GraphSegment glyph="space" status="LocalOnly" behind={behind} />
				</Row>
				{onTip.map((worktree) => (
					<WorktreeOnTip
						key={worktree.name}
						projectId={projectId}
						worktree={worktree}
						worktrees={worktrees}
						behind={behind}
					/>
				))}
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
										startsRail={index === 0 && onTip.length === 0}
										behind={behind}
										worktrees={worktrees}
										checkCommit={checkCommit}
										onAmendCommit={onAmendCommit}
										canAmendCommit={canAmendCommit}
										scrollElementRef={scrollElementRef}
										scrollPaddingEnd={scrollPaddingEnd}
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
										behind={behind}
										worktrees={worktrees}
										projectId={projectId}
										segment={segment}
										stackId={stack.id}
										checkCommit={checkCommit}
										onAmendCommit={onAmendCommit}
										canAmendCommit={canAmendCommit}
										scrollElementRef={scrollElementRef}
										scrollPaddingEnd={scrollPaddingEnd}
										stackScrollStart={stackScrollStart}
										stackSize={stackSize}
										segmentIndex={index}
										selectedCommitIndex={
											selectedSegmentIndex === index ? selectedCommitIndex : undefined
										}
									/>
								)}
							</div>
							<SegmentRailConnector projectId={projectId} segment={segment} behind={behind} />
						</Fragment>
					);
				})}
			</StackCard>
			<GraphGap height={CARD_GAP} bend="LocalOnly" />
		</div>
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
	graph: Graph;
	newBranch: NewBranchActions;
	checkCommit: (evt: { commitId: string; shiftKey: boolean }) => void;
	onAmendCommit: (commitId: string) => void;
	canAmendCommit: boolean;
	onEdgeSpill: (offset: -1 | 1) => void;
	/** The uncommitted files card, which heads the trunk above the stack cards. */
	head: ReactNode;
	/** Stands in for the card at the scroller's head while the card is scrolled out above. */
	dock: ReactNode;
	/** How far down the dock's mark sticks: the card's head height, so the stand-in takes over as the head is pushed. */
	dockOffset: number;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	scrollPaddingEnd: number;
}> = ({
	projectId,
	graph,
	newBranch,
	checkCommit,
	onAmendCommit,
	canAmendCommit,
	onEdgeSpill,
	head,
	dock,
	dockOffset,
	scrollElementRef,
	scrollPaddingEnd,
}) => {
	const addressSpace = useAddressSpace();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const selection = useSelection("applied", addressSpace);
	const activeList = useActiveList();
	const dispatch = useAppDispatch();
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
	// Cards in the graph's order, the section below.
	const { plan, stacks } = graph;
	// Undefined `headInfo` is still loading, which is not the same as "empty" —
	// treating it as empty would flash the empty state on every open.
	const isEmpty = headInfo !== undefined && stacks.length === 0;
	const foldedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectFoldedSegments(state, projectId),
	);
	const pendingPushBranchList = useMutationState({
		filters: {
			mutationKey: [projectId, "workspaceBranchAndAncestorsPush"],
			status: "pending",
		},
		select: (mutation) =>
			(mutation.state.variables as PayloadFor<"workspaceBranchAndAncestorsPush">).branch,
	});
	// React Compiler leaves components using useVirtualizer uncompiled, hence manual memo:
	// a fresh Set every render would re-render every stack.
	const pendingPushBranches = useMemo(
		() => new Set(pendingPushBranchList),
		[pendingPushBranchList],
	);
	const retainScrollElement = useCallback(
		(element: HTMLDivElement | null) => {
			if (element) scrollElementRef.current = element;
		},
		[scrollElementRef],
	);
	// The cards start under the uncommitted files card and the gap below it.
	const [headRef, headHeight] = useHeight();
	const scrollMargin = headHeight + CARD_GAP;
	const getStackKey = useCallback((index: number) => stacks[index]?.id ?? index, [stacks]);
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
	const selectedContext =
		selection?._tag === "Branch"
			? headInfoIndex?.branchContextByRefBytes(selection.branchRef)
			: selection?._tag === "Commit"
				? headInfoIndex?.commitContextByCommitId(selection.commitId)
				: undefined;
	const selectedStack =
		selectedContext === undefined ? undefined : headInfo?.stacks[selectedContext.stackIndex];
	const selectedStackIndexRaw = selectedStack === undefined ? -1 : stacks.indexOf(selectedStack);
	const selectedStackIndex = selectedStackIndexRaw === -1 ? undefined : selectedStackIndexRaw;
	const selectedSegmentIndex = selectedContext?.segmentIndex;
	const selectedCommitIndex =
		selection?._tag === "Commit"
			? headInfoIndex?.commitContextByCommitId(selection.commitId)?.commitIndex
			: undefined;

	// Pin the containing stack too, otherwise the nested selected row can still be unmounted.
	// In index order: a pinned index flipping between appended and in place would move every
	// card's node, and the scroller re-anchors on each move.
	const rangeExtractorWithSelected = useCallback(
		(range: Range) =>
			getRangeExtractorWithIndices(
				range,
				selectedStackIndex === undefined ? [] : [selectedStackIndex],
			).sort((a, b) => a - b),
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
			const stackPadHeight = 6;
			const stackBordersHeight = 2;
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
				stackPadHeight +
				stackBordersHeight +
				contentHeight +
				finalConnectorHeight +
				Math.max(0, stack.segments.length - 1) * betweenSegmentConnectorHeight +
				CARD_GAP
			);
		},
		getItemKey: getStackKey,
		rangeExtractor: rangeExtractorWithSelected,
		scrollMargin,
		// The head clears the docked uncommitted files row, the foot the docked target row.
		scrollPaddingStart: HEAD_DOCKED_HEIGHT,
		scrollPaddingEnd,
	});

	const selectedAddressKey = selection === null ? undefined : addressIdentityKey(selection);
	// Whether the selection sits in the section's fold, looked up once per
	// selection or plan: the hotkeys ask on every render.
	const selectedInSection = useMemo(
		() => selection !== null && inSection(plan, selection),
		[plan, selection],
	);
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

	const toggleIncoming = () => dispatch(projectSlice.actions.toggleGraphIncoming({ projectId }));
	const showMoreRun = (runId: string) =>
		dispatch(projectSlice.actions.showMoreGraphRun({ projectId, runId }));
	const foldRun = (runId: string) =>
		dispatch(projectSlice.actions.foldGraphRun({ projectId, runId }));
	const hotkeysRef = useRef<HTMLDivElement>(null);
	useActiveListsHotkeys({
		addressSpace,
		projectId,
		ref: hotkeysRef,
		checkCommit,
		focusCommitMessageInput,
		onEdgeSpill,
		pendingPushBranches,
		// The fold key on a section row closes the fold and parks the cursor on
		// the row above it, since the header is not a value.
		sectionToggle: selectedInSection
			? () => {
					const first = sectionAddresses(plan)[0];
					const index =
						first === undefined
							? undefined
							: addressSpace.indexByKey.get(addressIdentityKey(first));
					const above = index === undefined ? undefined : addressSpace.items[index - 1];
					if (above !== undefined) setCursor("applied", above);
					toggleIncoming();
				}
			: null,
	});

	return (
		<DryRunWorkspaceContext value={dryRunWorkspace}>
			<div
				ref={retainScrollElement}
				className={classes(uiStyles.scroller, styles.stacksScroller)}
				style={{ "--row-padding-inline-start": `${ROW_INSET}px` }}
			>
				{/* Its own tree: the files walk with their own cursor, and the arrow
				    keys spill into the cards' tree at its edge. */}
				<div ref={headRef}>{head}</div>
				<div className={styles.dock} style={{ "--dock-offset": `${dockOffset}px` }}>
					{dock}
				</div>
				<GraphGap height={CARD_GAP} />
				{/* One tree: the cards and the upstream section below them share the
				    applied list's cursor, and arrow keys walk them in reading order. */}
				<div
					tabIndex={0}
					role="tree"
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					className={classes(styles.tree, styles.content)}
					data-focus-scope={"sidebar" satisfies FocusScope}
					data-preview-source={activeList === "applied"}
					ref={useMergedRefs<HTMLDivElement>(
						hotkeysRef,
						useAutofocusScope(activeList === "applied"),
					)}
				>
					<div
						className={classes(styles.stacks, styles.virtualContainer)}
						ref={rowVirtualizer.containerRef}
					>
						{rowVirtualizer.getVirtualItems().map((virtualRow) => {
							const stack = stacks[virtualRow.index];
							if (stack === undefined) return null;

							return (
								<StackC
									key={stack.id ?? virtualRow.index}
									data-index={virtualRow.index}
									ref={rowVirtualizer.measureElement}
									projectId={projectId}
									stack={stack}
									checkCommit={checkCommit}
									onAmendCommit={onAmendCommit}
									canAmendCommit={canAmendCommit}
									pendingPushBranches={pendingPushBranches}
									scrollElementRef={scrollElementRef}
									scrollPaddingEnd={scrollPaddingEnd}
									stackScrollStart={virtualRow.start}
									stackSize={virtualRow.size}
									scrollMargin={scrollMargin}
									worktrees={plan.worktrees}
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
					{plan.worktrees.standalone.map((worktree) => (
						<WorktreeCard
							key={worktree.name}
							projectId={projectId}
							worktree={worktree}
							worktrees={plan.worktrees}
						/>
					))}
					<Section
						projectId={projectId}
						plan={plan}
						onToggleIncoming={toggleIncoming}
						onShowMoreRun={showMoreRun}
						onFoldRun={foldRun}
						scrollElementRef={scrollElementRef}
					/>
				</div>

				{isEmpty && (
					<div className={styles.empty}>
						<NoStacks projectId={projectId} newBranch={newBranch} />
					</div>
				)}
				<div className={styles.foot} />
			</div>
		</DryRunWorkspaceContext>
	);
};

export const WorkspaceLists: FC<
	{
		projectId: string;
		/** The stacks graph, computed once by the host that built the address space. */
		graph: Graph;
		addressSpace: AddressSpace<Address>;
		uncommittedAddressSpace: AddressSpace<string>;
		absorptionTargetCommitIds: ReadonlySet<string>;
		onActiveFileSelection: (selection: string) => void;
		stacksHeaderActions?: ReactNode;
		newBranch: NewBranchActions;
	} & ComponentProps<"div">
> = ({
	projectId,
	graph,
	addressSpace,
	uncommittedAddressSpace,
	absorptionTargetCommitIds,
	onActiveFileSelection,
	stacksHeaderActions,
	newBranch,
	...props
}) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: worktreeChanges } = useQuery(changesInWorktreeQueryOptions(projectId));
	const appliedSelection = useSelection("applied", addressSpace);
	// After the hook on purpose: a hook call between the index and the commit
	// target derivation below stops the compiler from memoizing that derivation,
	// and a fresh commit target every render re-renders the uncommitted list.
	const headInfoIndex = headInfo ? getHeadInfoIndex(headInfo) : undefined;
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

	const uncommittedFolded = useAppSelector((state) =>
		projectSlice.selectors.selectUncommittedFolded(state, projectId),
	);

	// The files and the cards are two lists in one scroller, so arrow navigation
	// continues across their boundary: entering a list selects its item nearest
	// the border, while the list being left keeps its selection. An empty
	// neighbour keeps focus where it is. The folded card has no rows to land on,
	// so arrow keys stop at the boundary rather than moving the selection into a
	// list nobody can see.
	const spillIntoStacks = (offset: -1 | 1) => {
		if (offset !== 1) return;
		const item = addressSpace.items.at(0);
		if (item === undefined) return;
		setCursor("applied", item);
		focusScope("sidebar");
	};
	const spillIntoUncommittedChanges = (offset: -1 | 1) => {
		if (offset !== -1 || uncommittedFolded) return;
		const path = uncommittedAddressSpace.items.at(-1);
		if (path === undefined) return;
		onActiveFileSelection(path);
		focusScope("uncommitted-files");
	};
	const scrollElementRef = useRef<HTMLDivElement>(null);
	// The docked target row's height, which the card's commit form sticks above; a row
	// scrolled into view clears it, else the foot's gradient.
	const footDock = graph.plan.header !== null ? DOCKED_HEIGHT : 0;
	const scrollPaddingEnd = Math.max(footDock, 14);
	// The card's head, whose height says when its docked stand-in takes over.
	const [cardHeadRef, cardHeadHeight] = useHeight();
	const uncommitted = (
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
							amendCommit={amendCommit}
							canAmendCommit={canAmendCommit}
							selectActiveFile={onActiveFileSelection}
							spillEdge={spillIntoStacks}
							worktreeChanges={worktreeChanges}
							scrollElementRef={scrollElementRef}
							footDock={footDock}
							headRef={cardHeadRef}
							headHeight={cardHeadHeight}
						/>
					}
				/>
			}
		/>
	);

	return (
		<WorkspaceListsProvider
			addressSpace={addressSpace}
			absorptionTargetCommitIds={absorptionTargetCommitIds}
		>
			<div {...props} className={classes(props.className, styles.lists)}>
				<SectionHeaderRow
					label="Workspace"
					className={styles.header}
					actions={stacksHeaderActions}
				/>
				<Stacks
					projectId={projectId}
					graph={graph}
					newBranch={newBranch}
					checkCommit={checkCommit}
					onAmendCommit={amendCommit}
					canAmendCommit={canAmendCommit}
					onEdgeSpill={spillIntoUncommittedChanges}
					head={uncommitted}
					dockOffset={cardHeadHeight}
					dock={
						<UncommittedChangesRow
							changes={worktreeChanges?.changes ?? []}
							isClean={worktreeChanges !== undefined && worktreeChanges.changes.length === 0}
							projectId={projectId}
							mode={{
								kind: "docked",
								onSelect: () => scrollElementRef.current?.scrollTo({ top: 0 }),
							}}
							className={styles.dockRow}
						/>
					}
					scrollElementRef={scrollElementRef}
					scrollPaddingEnd={scrollPaddingEnd}
				/>
			</div>
		</WorkspaceListsProvider>
	);
};
