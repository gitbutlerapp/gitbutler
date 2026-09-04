import rowStyles from "./Row.module.css";
import uiStyles from "#ui/components/ui.module.css";
import { useBranchRemove } from "#ui/api/mutations.ts";
import { decodeBytes, encodeBytes } from "#ui/api/bytes.ts";
import { assert } from "#ui/assert.ts";
import { branchIsEmpty, type BranchFilters } from "#ui/branch.ts";
import { commitIsDiverged, commitTitle } from "#ui/commit.ts";
import { classes } from "#ui/components/classes.ts";
import { EmptyState } from "#ui/components/EmptyState.tsx";
import {
	GraphSegment,
	type GraphSegmentGlyph,
	type GraphSegmentStatus,
} from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { branchesHotkeys, toElectronAccelerator } from "#ui/hotkeys.ts";
import {
	nativeMenuItem,
	nativeMenuSeparator,
	showNativeContextMenu,
	showNativeMenuFromTrigger,
	type NativeMenuItem,
} from "#ui/native-menu.ts";
import { branchAddress, commitAddress, addressIdentityKey, type Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAutofocusScope, useAddressSpaceHotkeys, type FocusScope } from "#ui/focus-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { getRangeExtractorWithIndices } from "#ui/virtual.ts";
import type { Commit, ListedBranch } from "@gitbutler/but-sdk";
import { Toolbar } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useHotkey } from "@tanstack/react-hotkeys";
import { type Range, useVirtualizer } from "@tanstack/react-virtual";
import {
	type ComponentProps,
	type FC,
	Fragment,
	type RefObject,
	useCallback,
	useLayoutEffect,
	useRef,
	useState,
} from "react";
import {
	Row,
	RowFoldToggle,
	RowLabel,
	RowLabelContainer,
	RowLabelGroup,
	RowMeta,
	RowMetaSeparator,
	RowToolbar,
	SectionHeaderRow,
} from "./Row.tsx";
import { ListFilterRow } from "./ListFilterRow.tsx";
import { useListFilter } from "./useListFilter.ts";
import {
	getRowButtonClassName,
	treeItemId,
	useIsSelected as useIsSelectedInList,
} from "./Row-utils.ts";
import { StackCard } from "./StackCard.tsx";
import stackCardStyles from "./StackCard.module.css";
import { emptyBranchesListContent, type BranchesListContent } from "./useBranchesList.ts";
import {
	startKeyboardTransfer,
	setCursor,
	useCursorWriteBack,
	useSelection,
} from "#ui/use-cursor.ts";
import { useApplyToWorkspace } from "./useApplyToWorkspace.ts";
import type { NewBranchActions } from "./useNewBranch.ts";
import styles from "./BranchesList.module.css";

/** The filter menu, in the order it is shown. */
const filterMenuLabels: Array<[keyof BranchFilters, string]> = [
	["showEmpty", "Include Empty Branches"],
	["onlyLocal", "Show Only Local Branches"],
	["onlyStacks", "Show Only Stacks"],
];

/**
 * The graph has no remote-only state, so a branch that exists only on a remote
 * takes the same glyph as a synced one: it has nothing unpushed, which
 * "LocalOnly" would wrongly imply.
 */
const branchGraphStatus = (branch: ListedBranch): GraphSegmentStatus =>
	branch.remoteRefs.length > 0 ? "LocalAndRemote" : "LocalOnly";

const useIsSelected = (address: Address): boolean => useIsSelectedInList(address, "unapplied");

const InertRow: FC<{ branch: ListedBranch; label: string }> = ({ branch, label }) => (
	<Row interactive={false} role="treeitem" aria-label={label}>
		<GraphSegment glyph="parent" status={branchGraphStatus(branch)} />
		<RowLabelContainer>
			<RowLabel className={rowStyles.fadedText}>{label}</RowLabel>
		</RowLabelContainer>
	</Row>
);

const CommitItem: FC<{
	commit: Commit;
	positionInSet: number;
	setSize: number;
}> = ({ commit, positionInSet, setSize }) => {
	const address = commitAddress({ commitId: commit.id, changeId: commit.changeId });
	const isSelected = useIsSelected(address);
	const title = commitTitle(commit.message);
	const copyCommit = () =>
		startKeyboardTransfer({ sources: [address], kind: "copy", placement: "above" });
	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			label: "Copy Commit",
			accelerator: toElectronAccelerator(branchesHotkeys.copy.hotkey),
			onSelect: copyCommit,
		}),
	];

	return (
		// oxlint-disable-next-line jsx-a11y/interactive-supports-focus -- This page was vibecoded and needs an accessibility pass.
		<Row
			id={treeItemId(address)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-level={2}
			aria-posinset={positionInSet}
			aria-setsize={setSize}
			aria-selected={isSelected}
			isSelected={isSelected}
			scrollSelectedIntoView={false}
			onSelect={() => setCursor("unapplied", address)}
			onContextMenu={(event) => void showNativeContextMenu(event, menuItems)}
		>
			<GraphSegment
				glyph="commit"
				status={commitIsDiverged(commit) ? "Diverged" : commit.state.type}
			/>
			<RowLabelContainer>
				<RowLabel singleLine>
					{title === undefined ? <span className={rowStyles.fadedText}>(no message)</span> : title}
				</RowLabel>
			</RowLabelContainer>
		</Row>
	);
};

const BranchCommits: FC<{
	branch: ListedBranch;
	commits: Array<Commit> | undefined;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	stackScrollStart: number;
	stackSize: number;
	branchAddressIndex: number;
	selectedCommitIndex: number | undefined;
}> = ({
	branch,
	commits,
	scrollElementRef,
	stackScrollStart,
	stackSize,
	branchAddressIndex,
	selectedCommitIndex,
}) => {
	const getCommitKey = useCallback((index: number) => commits?.[index]?.id ?? index, [commits]);
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
		count: commits?.length ?? 0,
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

	// Nested commit lists share the outer scroller, so the virtualizer needs this list's start in
	// scroller coordinates. A mounted list can move when an earlier branch changes without its own
	// DOM node changing; remeasure only when the stack moves or resizes, or when visible rows before
	// this branch change. This avoids forcing layout on ordinary virtualizer renders.
	useLayoutEffect(() => {
		const element = commitListRef.current;
		if (!element) return;

		const nextScrollMargin = stackScrollStart + element.offsetTop;
		setScrollMargin((currentScrollMargin) =>
			currentScrollMargin === nextScrollMargin ? currentScrollMargin : nextScrollMargin,
		);
	}, [branchAddressIndex, stackScrollStart, stackSize]);

	// Reveal the selected commit before paint when its resolved index changes. Activity reconnects
	// layout effects on reveal without changing that index, so remembering it preserves manual
	// scroll.
	const lastRevealedCommitIndexRef = useRef<number>(undefined);

	useLayoutEffect(() => {
		if (
			selectedCommitIndex !== undefined &&
			selectedCommitIndex !== lastRevealedCommitIndexRef.current
		)
			rowVirtualizer.scrollToIndex(selectedCommitIndex, { align: "auto" });

		lastRevealedCommitIndexRef.current = selectedCommitIndex;
	}, [rowVirtualizer, selectedCommitIndex]);

	if (commits === undefined) return <InertRow branch={branch} label="Loading…" />;

	if (commits.length === 0) return <InertRow branch={branch} label="No commits." />;

	return (
		// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- Tree items need ARIA group semantics.
		<div ref={containerRef} role="group" className={styles.virtualContainer}>
			{rowVirtualizer.getVirtualItems().map((virtualRow) => {
				const commit = commits[virtualRow.index];
				if (commit === undefined) return null;

				return (
					<div
						key={commit.id}
						data-index={virtualRow.index}
						ref={rowVirtualizer.measureElement}
						style={{
							position: "absolute",
							top: 0,
							left: 0,
							width: "100%",
							height: virtualRow.size,
						}}
					>
						<CommitItem
							commit={commit}
							positionInSet={virtualRow.index + 1}
							setSize={commits.length}
						/>
					</div>
				);
			})}
		</div>
	);
};

const BranchItem: FC<{
	projectId: string;
	branch: ListedBranch;
	isTopBranch: boolean;
	isStacked: boolean;
	commits: Array<Commit> | undefined;
	scrollElementRef: RefObject<HTMLDivElement | null>;
	stackScrollStart: number;
	stackSize: number;
	branchAddressIndex: number;
	selectedCommitIndex: number | undefined;
	positionInSet: number;
	setSize: number;
}> = ({
	projectId,
	branch,
	isTopBranch,
	isStacked,
	commits,
	scrollElementRef,
	stackScrollStart,
	stackSize,
	branchAddressIndex,
	selectedCommitIndex,
	positionInSet,
	setSize,
}) => {
	const dispatch = useAppDispatch();
	const branchRef = branch.refName.full;
	const address = branchAddress({ branchRef: encodeBytes(branchRef) });
	// A branch with no commits of its own has nothing to unfold; an unknown
	// count keeps the affordance.
	const canUnfold = !branchIsEmpty(branch);
	const unfolded =
		useAppSelector((state) =>
			projectSlice.selectors.selectBranchUnfolded(state, projectId, branchRef),
		) && canUnfold;
	const isSelected = useIsSelected(address);
	const [now] = useState(() => Date.now());

	// Same topology as the applied list: nothing above the branch means the
	// rail turns in from the right, otherwise it joins the branch above it. This
	// describes where the branch sits in the stack, so it does not change with
	// fold state.
	const railGlyph: GraphSegmentGlyph = isTopBranch ? "forkRight" : "joinRight";

	const review = branch.review;

	const { isPending: isApplyPending, apply } = useApplyToWorkspace(projectId);
	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove(projectId);

	const toggleUnfolded = () => {
		dispatch(projectSlice.actions.toggleBranchUnfolded({ projectId, branchRef }));
	};

	const openReviewInBrowser = async (): Promise<void> => {
		if (review) await window.lite.openInWebBrowser(review.htmlUrl);
	};

	const menuItems: Array<NativeMenuItem> = [
		nativeMenuItem({
			// Branches run from the tip down, so applying the top branch of a stack
			// brings the whole stack with it — the label says so.
			label: isTopBranch && isStacked ? "Apply Stack to Workspace" : "Apply to Workspace",
			enabled: !isApplyPending,
			onSelect: () => apply(branchRef),
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Open Pull Request In Browser",
			enabled: review !== null,
			onSelect: openReviewInBrowser,
		}),
		nativeMenuSeparator,
		nativeMenuItem({
			label: "Delete Branch Reference",
			enabled: branch.hasLocal && !isBranchRemovePending,
			accelerator: toElectronAccelerator(branchesHotkeys.deleteBranchRef.hotkey),
			onSelect: () => branchRemove({ projectId, refName: encodeBytes(branchRef) }),
		}),
	];

	const lastAuthorName = branch.lastAuthor?.name;

	const showsAuthorMeta = lastAuthorName !== undefined || branch.updatedAtMs !== null;
	/* The branch's own commits, matching what unfolding reveals.
	   commitsAheadOfTarget would also count the branches below it in a stack, so
	   every row above the bottom would overstate. Only while folded: the count
	   stands in for the commits it hides, so showing it alongside them would just
	   be noise. */
	const showsCommitCount = !unfolded && branch.commitCount !== null && branch.commitCount > 0;

	return (
		<div
			id={treeItemId(address)}
			role="treeitem"
			aria-label={branch.displayName}
			aria-level={1}
			aria-posinset={positionInSet}
			aria-setsize={setSize}
			aria-selected={isSelected}
			// A branch with nothing to unfold is a leaf: omit the attribute
			// entirely rather than reporting it as collapsed.
			aria-expanded={canUnfold ? unfolded : undefined}
		>
			<Row
				isSelected={isSelected}
				onSelect={() => setCursor("unapplied", address)}
				onContextMenu={(event) => {
					void showNativeContextMenu(event, menuItems);
				}}
			>
				{canUnfold ? (
					<RowFoldToggle
						folded={!unfolded}
						aria-label={unfolded ? "Fold commits" : "Unfold commits"}
						onClick={toggleUnfolded}
						glyph={<GraphSegment glyph={railGlyph} status={branchGraphStatus(branch)} />}
						foldedIndicator={<GraphSegment glyph="group" status={branchGraphStatus(branch)} />}
					/>
				) : (
					<GraphSegment glyph={railGlyph} status={branchGraphStatus(branch)} />
				)}

				<RowLabelGroup>
					<RowLabelContainer>
						<RowLabel heading singleLine title={branch.displayName}>
							{branch.displayName}
						</RowLabel>
					</RowLabelContainer>

					<RowMeta>
						{showsAuthorMeta && (
							<span
								className={classes(
									rowStyles.fadedText,
									rowStyles.metaItem,
									rowStyles.metaItemShrinkable,
								)}
								title={branch.lastAuthor?.email}
							>
								<span className={rowStyles.metaItemText}>
									{lastAuthorName !== undefined && <>{lastAuthorName} </>}
									{branch.updatedAtMs !== null && (
										<RelativeTime timestamp={branch.updatedAtMs} now={now} />
									)}
								</span>
							</span>
						)}

						{showsCommitCount && (
							<>
								{showsAuthorMeta && <RowMetaSeparator />}
								<span className={classes(rowStyles.fadedText, rowStyles.metaItem)}>
									<Icon size={14} name="commit" />
									{branch.commitCount}
								</span>
							</>
						)}

						{review !== null && (
							<>
								{(showsAuthorMeta || showsCommitCount) && <RowMetaSeparator />}
								<span
									title={review.title}
									className={classes(rowStyles.fadedText, rowStyles.metaItem)}
								>
									<Icon size={14} name="pr" />
									{review.unitSymbol}
									{review.number}
								</span>
							</>
						)}

						{isApplyPending && <Icon name="spinner" />}
					</RowMeta>
				</RowLabelGroup>

				<Toolbar.Root aria-label="Branch actions" render={<RowToolbar />}>
					<Toolbar.Button
						aria-label="Branch menu"
						onClick={(event) => {
							void showNativeMenuFromTrigger(event.currentTarget, menuItems);
						}}
						className={getRowButtonClassName({ iconOnly: true })}
					>
						<Icon name="kebab" />
					</Toolbar.Button>
				</Toolbar.Root>
			</Row>

			{unfolded && (
				<BranchCommits
					branch={branch}
					commits={commits}
					scrollElementRef={scrollElementRef}
					stackScrollStart={stackScrollStart}
					stackSize={stackSize}
					branchAddressIndex={branchAddressIndex}
					selectedCommitIndex={selectedCommitIndex}
				/>
			)}
		</div>
	);
};

export const BranchesList: FC<
	{
		projectId: string;
		branches: BranchesListContent | undefined;
		isPending: boolean;
		isError: boolean;
		/**
		 * Owned by the sidebar and shared with its unapplied header, so both `+`
		 * buttons offer the same menu and see the same create in flight.
		 */
		newBranch: NewBranchActions;
	} & ComponentProps<"div">
> = ({ projectId, branches, isPending, isError, newBranch, ...restProps }) => {
	const dispatch = useAppDispatch();

	// WorkspacePage resolves selection from this query data and passes the same data down, so
	// selection and rendering consume the same snapshot.
	const { stacks, stackIndexByAddressIndex, addressSpace } = branches ?? emptyBranchesListContent;
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	const search = useAppSelector((state) =>
		projectSlice.selectors.selectBranchSearch(state, projectId),
	);
	const noOperationPending = useAppSelector(
		(state) => projectSlice.selectors.selectPendingOperation(state, projectId)._tag === "None",
	);

	// `onlyStacks` is the one filter that hides branches the resting list would
	// show — the other two default to narrowing and only ever widen from there —
	// so it, and a search, are what make an empty list a no-match rather than a
	// state at rest.
	const isNarrowed = (search ?? "").trim() !== "" || filters.onlyStacks;
	const isEmptyAtRest = stacks.length === 0 && !isPending && !isError && !isNarrowed;

	const selection = useSelection("unapplied", addressSpace);
	useCursorWriteBack("unapplied", addressSpace);

	const panelRef = useRef<HTMLDivElement>(null);
	const treeRef = useRef<HTMLDivElement>(null);
	const scrollElementRef = useRef<HTMLDivElement>(null);

	// Activity preserves the scroller DOM but temporarily clears its ref; nested virtualizer
	// effects reconnect before that ref is restored. Keep the last node available to them. A real
	// unmount discards this ref, and the stable callback avoids ref churn on virtualizer renders.
	const retainScrollElement = useCallback((element: HTMLDivElement | null) => {
		if (element) scrollElementRef.current = element;
	}, []);

	// The virtualiser treats getItemKey identity as part of its measurement model. Keep it stable
	// across selection renders, but refresh estimates when either stack identity or commit counts
	// change.
	const getStackKey = useCallback(
		(index: number) => stacks[index]?.branches[0]?.branch.refName.full ?? index,
		[stacks],
	);
	const selectedAddressKey = selection === null ? undefined : addressIdentityKey(selection);
	const selectedAddressIndex =
		selectedAddressKey === undefined ? undefined : addressSpace.indexByKey.get(selectedAddressKey);
	const selectedStackIndex =
		selectedAddressIndex === undefined ? undefined : stackIndexByAddressIndex[selectedAddressIndex];
	const rangeExtractorWithSelected = useCallback(
		(range: Range) =>
			getRangeExtractorWithIndices(
				range,
				selectedStackIndex === undefined ? [] : [selectedStackIndex],
			),
		[selectedStackIndex],
	);

	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const rowVirtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: stacks.length,
		getScrollElement: () => scrollElementRef.current,
		estimateSize: (index) => {
			// Keep in sync with Row.module.css and StackCard.module.css.
			const singleLineRowHeight = 28;
			const branchMetaLineHeight = 20;
			const branchMetaPaddingEnd = 6;
			const stackBodyPaddingStart = 6;
			const stackBorderHeight = 1;
			const stackFinalConnectorHeight = 8;
			const stackBetweenBranchConnectorHeight = 14;

			const branchCount = stacks[index]?.branches.length ?? 0;
			const commitCount = stacks[index]?.commitCount ?? 0;

			return (
				stackBodyPaddingStart +
				stackBorderHeight +
				stackFinalConnectorHeight +
				branchCount * (singleLineRowHeight + branchMetaLineHeight + branchMetaPaddingEnd) +
				commitCount * singleLineRowHeight +
				Math.max(0, branchCount - 1) * stackBetweenBranchConnectorHeight
			);
		},
		getItemKey: getStackKey,
		rangeExtractor: rangeExtractorWithSelected,
		// Matches --scroll-gradient-height.
		scrollPaddingStart: 14,
		scrollPaddingEnd: 14,
	});

	// Activity reconnects layout effects on reveal without changing the selection. Remember the
	// handled address so revealing the tab preserves manual scroll.
	const lastRevealedAddressKeyRef = useRef<string>(undefined);

	// If the selected row's stack is not rendered, scroll the outer virtualizer to it. The row or
	// nested virtualizer then handles precise alignment.
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

	const { isPending: isBranchRemovePending, mutate: branchRemove } = useBranchRemove(projectId);
	const selectedBranchIsLocal =
		selection?._tag === "Branch" && decodeBytes(selection.branchRef).startsWith("refs/heads/");
	const removeBranch = (branchRef: Array<number>) => {
		branchRemove({ projectId, refName: branchRef });
	};

	useAddressSpaceHotkeys({
		projectId,
		addressSpace,
		group: "Sidebar",
		select: (newItem) => setCursor("unapplied", newItem),
		selection,
		selectSectionPredicate: (address) => address._tag === "Branch",
		ref: treeRef,
		getKey: addressIdentityKey,
	});

	useHotkey(
		branchesHotkeys.copy.hotkey,
		() => {
			if (selection?._tag !== "Commit") return;
			startKeyboardTransfer({ sources: [selection], kind: "copy", placement: "above" });
		},
		{
			conflictBehavior: "allow",
			enabled: noOperationPending && selection?._tag === "Commit",
			ignoreInputs: true,
			meta: branchesHotkeys.copy.meta,
			target: treeRef,
		},
	);

	useHotkey(
		branchesHotkeys.deleteBranchRef.hotkey,
		() => {
			if (selection?._tag === "Branch") removeBranch(selection.branchRef);
		},
		{
			enabled: selectedBranchIsLocal && !isBranchRemovePending,
			meta: branchesHotkeys.deleteBranchRef.meta,
			target: treeRef,
		},
	);

	const firstBranch = stacks[0]?.branches[0]?.branch;
	const branchFilter = useListFilter({
		filter: search,
		setFilter: (search) => dispatch(projectSlice.actions.setBranchSearch({ projectId, search })),
		inputId: "branches-filter-input",
		subject: "branches",
		scope: "sidebar",
		selectionKey: selection === null ? null : addressIdentityKey(selection),
		firstKey:
			firstBranch === undefined
				? undefined
				: addressIdentityKey(branchAddress({ branchRef: encodeBytes(firstBranch.refName.full) })),
		onEnterList: () => {
			if (selection !== null) setCursor("unapplied", selection);
		},
		panelRef,
		listRef: treeRef,
	});

	const showFilterMenu = (trigger: HTMLElement) => {
		void showNativeMenuFromTrigger(
			trigger,
			filterMenuLabels.map(([filter, label]) =>
				nativeMenuItem({
					label,
					checked: filters[filter],
					onSelect: () => {
						dispatch(projectSlice.actions.toggleBranchFilter({ projectId, filter }));
					},
				}),
			),
		);
	};

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)} ref={panelRef}>
			{branchFilter.rowProps === null ? (
				<SectionHeaderRow
					className={styles.header}
					label="Recent branches"
					actions={
						<Toolbar.Root aria-label="Branch list actions" render={<RowToolbar forceVisible />}>
							<Toolbar.Group className={styles.headerGroup}>
								<Toolbar.Button
									aria-label="Branch filters"
									className={getRowButtonClassName({ size: "regular", iconOnly: true })}
									onClick={(evt) => showFilterMenu(evt.currentTarget)}
								>
									<Icon name="filter" />
								</Toolbar.Button>

								<Toolbar.Button
									aria-label="Filter branches"
									className={getRowButtonClassName({ size: "regular", iconOnly: true })}
									onClick={branchFilter.open}
								>
									<Icon name="search" />
								</Toolbar.Button>
							</Toolbar.Group>

							<Toolbar.Separator className={styles.headerSeparator} />

							<Toolbar.Button
								aria-label="New branch"
								className={getRowButtonClassName({ size: "regular", iconOnly: true })}
								onClick={(evt) => {
									void showNativeMenuFromTrigger(evt.currentTarget, newBranch.menuItems);
								}}
							>
								{newBranch.isPending ? <Icon name="spinner" /> : <Icon name="plus" />}
							</Toolbar.Button>
						</Toolbar.Root>
					}
				/>
			) : (
				<ListFilterRow {...branchFilter.rowProps} />
			)}

			<div
				ref={retainScrollElement}
				className={classes(uiStyles.scroller, styles.list)}
				data-empty={isEmptyAtRest}
			>
				{/* Loading, failing and narrowed-to-nothing all stay one line where the
				    rows would be: none of them is a surface at rest, and an answer about
				    a filter belongs next to the filter that caused it. Only a list that
				    is empty with nothing narrowing it gets the block. */}
				{stacks.length === 0 &&
					(isPending ? (
						<p className={classes("text-13", styles.msg)}>Loading branches…</p>
					) : isError ? (
						<p className={classes("text-13", styles.msg)}>Unable to load branches.</p>
					) : isNarrowed ? (
						<p className={classes("text-13", styles.msg)}>No matching branches.</p>
					) : (
						<EmptyState
							illustration="cactus"
							title="No branches here"
							// No button: the header's `+` already starts one, and it is the
							// only action this state has.
							description="Branches you are not working on show up in this list"
						/>
					))}

				<div
					tabIndex={0}
					role="tree"
					aria-label="Branches"
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					data-focus-scope={"sidebar" satisfies FocusScope}
					className={classes(styles.tree, styles.virtualContainer)}
					ref={useMergedRefs(rowVirtualizer.containerRef, treeRef, useAutofocusScope())}
				>
					{rowVirtualizer.getVirtualItems().map((virtualRow) => {
						const stack = stacks[virtualRow.index];
						if (stack === undefined) return null;

						return (
							<StackCard
								key={assert(stack.branches[0]).branch.refName.full}
								data-index={virtualRow.index}
								ref={rowVirtualizer.measureElement}
								style={{
									position: "absolute",
									top: 0,
									left: 0,
									width: "100%",
									transform: `translateY(${virtualRow.start}px)`,
								}}
								// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A stack is an ARIA group of tree items.
								role="group"
								aria-label="Stack"
							>
								{stack.branches.map(({ branch, addressIndex, commits }, index) => {
									const selectedCommitIndex =
										selectedAddressIndex !== undefined &&
										selectedAddressIndex > addressIndex &&
										selectedAddressIndex <= addressIndex + (commits?.length ?? 0)
											? selectedAddressIndex - addressIndex - 1
											: undefined;

									return (
										<Fragment key={branch.refName.full}>
											<BranchItem
												projectId={projectId}
												branch={branch}
												isTopBranch={index === 0}
												isStacked={stack.branches.length > 1}
												commits={commits}
												scrollElementRef={scrollElementRef}
												stackScrollStart={virtualRow.start}
												stackSize={virtualRow.size}
												branchAddressIndex={addressIndex}
												selectedCommitIndex={selectedCommitIndex}
												positionInSet={index + 1}
												setSize={stack.branches.length}
											/>

											{/* Carries the rail down to the next branch, and past the
											    last one as the card's floor — as the workspace card's
											    segment connectors do. */}
											<Row interactive={false} className={stackCardStyles.railConnector}>
												<GraphSegment glyph="parent" status={branchGraphStatus(branch)} />
											</Row>
										</Fragment>
									);
								})}
							</StackCard>
						);
					})}
				</div>
			</div>
		</div>
	);
};
