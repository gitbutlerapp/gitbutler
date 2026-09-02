import rowStyles from "./Row.module.css";
import { setCursor, useCursorWriteBack, useSelection } from "#ui/use-cursor.ts";
import uiStyles from "#ui/components/ui.module.css";
import { commitTitle } from "#ui/commit.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import {
	GraphSegment,
	type GraphSegmentGlyph,
	type GraphSegmentStatus,
} from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { commitAddress, addressIdentityKey, type Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAutofocusScope, useAddressSpaceHotkeys, type FocusScope } from "#ui/focus-scopes.ts";
import { useAppDispatch } from "#ui/store.ts";
import { RelativeTime } from "#ui/components/RelativeTime.tsx";
import { getRangeExtractorWithIndices } from "#ui/virtual.ts";
import {
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { Button } from "@base-ui/react";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { type Range, useVirtualizer } from "@tanstack/react-virtual";
import {
	type ComponentProps,
	type FC,
	useCallback,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { Row, RowLabel, RowLabelContainer, RowLabelFooter, SectionHeaderRow } from "./Row.tsx";
import { treeItemId, useIsSelected as useIsSelectedInList } from "./Row-utils.ts";
import type {
	UpstreamBranchItem,
	UpstreamCommitItem,
	UpstreamListItem,
	UpstreamListData,
} from "./useUpstreamList.ts";
import styles from "./UpstreamList.module.css";

const pluralRules = new Intl.PluralRules("en");

const useIsSelected = (address: Address): boolean => useIsSelectedInList(address, "upstream");

const upstreamCommitAddress = (item: UpstreamCommitItem): Address =>
	commitAddress({
		commitId: item.commit.id,
		// This is a hack that should be revisited...
		changeId: item.commit.changeId ?? item.commit.id,
	});

/**
 * The target branch the incoming commits below it belong to. It heads the card
 * the way a stack's own name heads a stack card, and starts the target rail
 * the commit rows hang off.
 */
const TargetHeadRow: FC<{ label: string }> = ({ label }) => (
	// Presentational within the tree: only the commit rows are `treeitem`s, so
	// everything else the card draws is excluded from the tree's children.
	<Row role="none" interactive={false}>
		<GraphSegment glyph="forkRight" status="Upstream" />
		<RowLabelContainer>
			<RowLabel heading singleLine title={label}>
				{label}
			</RowLabel>
		</RowLabelContainer>
	</Row>
);

const TargetCommitRow: FC<{
	item: UpstreamCommitItem;
	positionInSet: number;
	setSize: number;
	/**
	 * Overrides the rail colour the commit's own position would give it. The
	 * older section is one unbroken length of target line, so its rows keep the
	 * target's colour rather than each reporting that the workspace has them —
	 * which is true of every row there, and so tells the reader nothing.
	 */
	status?: GraphSegmentStatus;
}> = ({ item, positionInSet, setSize, status }) => {
	const { commit, review, inWorkspace } = item;
	const address = upstreamCommitAddress(item);
	const isSelected = useIsSelected(address);
	// A commit that landed a review is shown as that review: its title says
	// what changed, where "Merge pull request #N from …" only says that it did.
	const title = review?.title ?? commitTitle(commit.message);
	const [now] = useState(() => Date.now());

	const authorName = commit.author.name;

	return (
		<Row
			id={treeItemId(address)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-level={1}
			aria-posinset={positionInSet}
			aria-setsize={setSize}
			aria-selected={isSelected}
			isSelected={isSelected}
			scrollSelectedIntoView={false}
			onSelect={() => setCursor("upstream", address)}
		>
			<GraphSegment glyph="commit" status={status ?? (inWorkspace ? "Integrated" : "Upstream")} />
			<div className={styles.label}>
				<RowLabelContainer>
					{/* Commits the workspace already has are not dimmed: the row is
					    selectable like any other, and the rail's colour already
					    says which side of the boundary the commit is on. */}
					<RowLabel singleLine>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
					</RowLabel>
				</RowLabelContainer>
				<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
					<span
						className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						title={commit.author.email}
					>
						{authorName !== "" && <>{authorName} </>}
						<RelativeTime timestamp={commit.committedAt} now={now} />
					</span>

					{review !== null && (
						<span
							title={review.title}
							className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						>
							<Icon name="pr" />
							{review.unitSymbol}
							{review.number}
						</span>
					)}
				</RowLabelFooter>
			</div>
		</Row>
	);
};

/**
 * A workspace branch positioned against the target line. It carries its name
 * and nothing else — the rows exist to show where the workspace's branches
 * fork from the target, and everything actionable about a branch lives on the
 * workspace tab — except for having landed, which the rail's colour and a
 * second line both report.
 */
const UpstreamBranchRow: FC<{ item: UpstreamBranchItem; glyph: GraphSegmentGlyph }> = ({
	item,
	glyph,
}) => (
	<Row role="none" interactive={false}>
		<GraphSegment glyph={glyph} status={item.integrated ? "Integrated" : "LocalOnly"} />
		<div className={styles.label}>
			<RowLabelContainer>
				<RowLabel heading singleLine title={item.name}>
					{item.name}
				</RowLabel>
			</RowLabelContainer>

			{/* The rail's colour is easy to miss on a branch that has landed, so
			    the state is spelled out on the second line as well. */}
			{item.integrated && (
				<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
					<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>integrated</span>
				</RowLabelFooter>
			)}
		</div>
	</Row>
);

/**
 * A toggle for the shared history between two fork points: target commits the
 * workspace already has, whose count measures how much older one stack's base
 * is than the one above it.
 */
const SegmentExpanderRow: FC<{
	projectId: string;
	segmentId: string;
	count: number;
	expanded: boolean;
	/** Set when the row opens the workspace rail, so nothing is drawn above it. */
	opensRail: boolean;
}> = ({ projectId, segmentId, count, expanded, opensRail }) => {
	const dispatch = useAppDispatch();

	return (
		<div role="none" className={styles.expanderRow}>
			<div className={styles.expanderRail}>
				{/* The stacked rings stand in for the commits while they are folded
				    away; once they are on screen the rail just runs past the toggle. */}
				<GraphSegment
					className={styles.expanderGlyph}
					glyph={
						opensRail ? (expanded ? "parentHead" : "groupHead") : expanded ? "parent" : "group"
					}
					status="LocalOnly"
				/>
				<GraphSegment className={styles.railStub} glyph="parent" status="LocalOnly" />
			</div>
			<Button
				aria-expanded={expanded}
				className={classes(
					// Filled while the run it reveals is on screen, so the toggle
					// reads as pressed rather than as another thing to click.
					getButtonClassName({ variant: expanded ? "gray" : "outline", size: "small" }),
					styles.expanderButton,
				)}
				title="Target commits your workspace already has, between these two fork points."
				onClick={() =>
					dispatch(projectSlice.actions.toggleUpstreamSegment({ projectId, segmentId }))
				}
			>
				{expanded
					? "hide commits"
					: `${count} commit${pluralRules.select(count) === "one" ? "" : "s"} between`}
			</Button>
		</div>
	);
};

/**
 * The prose and the button that act on the whole target section. Its own band
 * between the incoming commits and the workspace's branches, off the graph
 * entirely: what it does is rebase every stack, not something that hangs off a
 * point on the target line.
 */
const UpdateBlock: FC<{
	incomingCount: number;
	hasIntegrated: boolean;
	canUpdate: boolean;
	isUpdatePending: boolean;
	onUpdateWorkspace: () => void;
}> = ({ incomingCount, hasIntegrated, canUpdate, isUpdatePending, onUpdateWorkspace }) => {
	const messageClassName = classes("text-12", "text-body", rowStyles.fadedText);

	return (
		<div role="none" className={styles.block}>
			{incomingCount > 0 ? (
				<p className={messageClassName}>
					Your workspace is {incomingCount} commit
					{pluralRules.select(incomingCount) === "one" ? "" : "s"} behind the upstream. Rebase to
					update all stacks at once.
				</p>
			) : hasIntegrated ? (
				<p className={messageClassName}>
					Integrated branches can be cleaned up by updating the workspace.
				</p>
			) : canUpdate ? (
				<p className={messageClassName}>
					Your stacks already contain the latest upstream commits. Update to advance the workspace
					base.
				</p>
			) : (
				<p className={messageClassName}>Your workspace is up to date.</p>
			)}
			<button
				type="button"
				className={getButtonClassName({ variant: "gray" })}
				disabled={!canUpdate}
				onClick={onUpdateWorkspace}
			>
				{isUpdatePending
					? "Updating…"
					: incomingCount > 0
						? "Rebase all stacks"
						: "Update workspace"}
			</button>
		</div>
	);
};

/**
 * Pages the older section further down the target line. Owns the fetching
 * rather than taking it from the list: the list's result is memoized on
 * its inputs, and a callback in it would defeat that.
 *
 * Renders nothing once the line is exhausted, so the band it draws is also the
 * answer to whether there is more to see — except when the walk failed, which
 * is reported instead, since a silently absent band would read as "no more
 * history" and leave nothing to retry with.
 */
/**
 * Asks for the next page of older history. The pages query is disabled, so
 * this button is the only thing that ever fetches it: the first press opens
 * the section, later ones extend it.
 */
const LoadMoreOlder: FC<{ projectId: string; hasOlder: boolean }> = ({ projectId, hasOlder }) => {
	// Shares the base listing the sidebar already reads; this only takes the
	// cursor its last commit supplies.
	const { data: olderFrom = null } = useQuery({
		...workspaceTargetCommitsQueryOptions(projectId),
		select: (page) => page.commits.at(-1)?.commit.id ?? null,
	});
	const { fetchNextPage, isFetching, isError } = useInfiniteQuery(
		olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom ?? ""),
	);

	if (olderFrom === null || (!hasOlder && !isError)) return null;

	return (
		<div role="none" className={styles.block}>
			{isError && (
				<p className={classes("text-12", "text-body", rowStyles.fadedText)}>
					Unable to load older commits.
				</p>
			)}
			<button
				type="button"
				className={getButtonClassName({ variant: "outline" })}
				disabled={isFetching}
				onClick={() => void fetchNextPage()}
			>
				{/* Names what it pages in rather than saying "more": it closes a run
				    nothing titles, so it is the only thing that says what is down
				    there. */}
				{isFetching ? <Icon name="spinner" /> : isError ? "Try again" : "Load older commits"}
			</button>
		</div>
	);
};

/**
 * A clipped stub carrying a rail past the first or last row of its region, so
 * the graph runs out into the section's own padding rather than stopping dead
 * at a row's edge.
 */
const RailEdge: FC<{ status: GraphSegmentStatus; edge: "head" | "tail" }> = ({ status, edge }) => (
	<div
		aria-hidden
		className={classes(styles.railEdge, edge === "head" ? styles.railHead : styles.railTail)}
	>
		<GraphSegment glyph="parent" status={status} />
	</div>
);

/**
 * The air closing an expanded shared-history run, answering the space its
 * toggle gets above it so the run reads as one block rather than as commits
 * crowding the branch below.
 */
const RunTail: FC = () => (
	<div aria-hidden className={styles.runTail}>
		<GraphSegment className={styles.railStub} glyph="parent" status="LocalOnly" />
	</div>
);

type VirtualRow =
	| { type: "sectionStart"; key: string }
	| { type: "sectionEnd"; key: string }
	| { type: "regionBreak"; key: string }
	| { type: "targetHead"; key: string; label: string }
	| { type: "message"; key: string; message: string }
	| {
			type: "item";
			key: string;
			item: UpstreamListItem;
			opensRail: boolean;
			status?: GraphSegmentStatus;
	  }
	| { type: "railEdge"; key: string; status: GraphSegmentStatus; edge: "head" | "tail" }
	| { type: "runTail"; key: string }
	| { type: "update"; key: string };

const itemKey = (item: UpstreamListItem): string => {
	switch (item.type) {
		case "commit":
			return `commit-${item.commit.id}`;
		case "branch":
			return `branch-${item.stackKey}/${item.name}`;
		case "expander":
			return `expander-${item.segmentId}`;
		default:
			return item satisfies never;
	}
};

export const UpstreamList: FC<
	{
		projectId: string;
		list: UpstreamListData;
		canUpdateWorkspace: boolean;
		isUpdatePending: boolean;
		onUpdateWorkspace: () => void;
	} & ComponentProps<"div">
> = ({ projectId, list, canUpdateWorkspace, isUpdatePending, onUpdateWorkspace, ...restProps }) => {
	// Derived once in WorkspacePage and passed down, so the rendered list and the
	// address space that resolves selection are the same object.
	const {
		items,
		incomingItemCount,
		olderItems,
		hasOlder,
		targetLabel,
		incomingCount,
		hasIntegrated,
		addressSpace,
		isPending,
		isError,
	} = list;

	const selection = useSelection("upstream", addressSpace);
	useCursorWriteBack("upstream", addressSpace);

	const hotkeysRef = useRef<HTMLDivElement>(null);
	const scrollElementRef = useRef<HTMLDivElement>(null);
	const retainScrollElement = useCallback((element: HTMLDivElement | null) => {
		if (element) scrollElementRef.current = element;
	}, []);

	useAddressSpaceHotkeys({
		projectId,
		addressSpace,
		group: "Sidebar",
		select: (newItem) => setCursor("upstream", newItem),
		selection,
		ref: hotkeysRef,
		getKey: addressIdentityKey,
	});

	// Decided by `canUpdateWorkspace`, not by whether this page has rows to
	// show: the base can be behind with nothing to list.
	const canUpdate = canUpdateWorkspace;

	// This component cannot be compiled by React Compiler due to the virtualiser - see the lint
	// ignore - hence the manual memo.
	const { virtualRows, virtualIndexByAddressKey } = useMemo(() => {
		const targetItems = items.slice(0, incomingItemCount);
		const workspaceItems = items.slice(incomingItemCount);
		// Whatever comes first opens the workspace rail, fold toggle or branch alike;
		// everything under it joins a rail that is already running.
		const railHead = workspaceItems[0];
		// The rail's tail carries on from the last branch, so it has to be coloured
		// the same as the segment it continues rather than always as local work.
		const lastBranch = workspaceItems.findLast((item) => item.type === "branch");
		// The listing parts once, under everything about what is coming in. Whichever
		// region opens what is below carries that break.
		const breaksFromIncoming = workspaceItems.length > 0 ? "workspace" : "older";

		const virtualRows: Array<VirtualRow> = [{ type: "sectionStart", key: "target-start" }];
		if (targetLabel !== null)
			virtualRows.push({ type: "targetHead", key: "target-head", label: targetLabel });
		if (isError) {
			virtualRows.push({
				type: "message",
				key: "error",
				message: "Unable to load incoming commits.",
			});
		}
		if (!isError && isPending && items.length === 0) {
			virtualRows.push({
				type: "message",
				key: "loading",
				message: "Loading incoming commits…",
			});
		}
		if (!isError && !isPending && targetLabel === null) {
			virtualRows.push({
				type: "message",
				key: "no-target",
				message: "No target branch is configured for this project.",
			});
		}
		for (const item of targetItems) {
			virtualRows.push({
				type: "item",
				key: `target-${itemKey(item)}`,
				item,
				opensRail: false,
			});
		}
		if (targetLabel !== null) {
			virtualRows.push({
				type: "railEdge",
				key: "target-tail",
				status: "Upstream",
				edge: "tail",
			});
		}
		virtualRows.push({ type: "sectionEnd", key: "target-end" });

		if (!isError && !isPending && targetLabel !== null)
			virtualRows.push({ type: "update", key: "update" });

		if (workspaceItems.length > 0) {
			if (breaksFromIncoming === "workspace")
				virtualRows.push({ type: "regionBreak", key: "workspace-break" });
			virtualRows.push({ type: "sectionStart", key: "workspace-start" });
			for (const [index, item] of workspaceItems.entries()) {
				virtualRows.push({
					type: "item",
					key: `workspace-${itemKey(item)}`,
					item,
					opensRail: item === railHead,
				});
				const next = workspaceItems[index + 1];
				if (item.type === "commit" && next !== undefined && next.type !== "commit")
					virtualRows.push({ type: "runTail", key: `${item.commit.id}-tail` });
			}
			virtualRows.push({
				type: "railEdge",
				key: "workspace-tail",
				status: lastBranch?.integrated === true ? "Integrated" : "LocalOnly",
				edge: "tail",
			});
			virtualRows.push({ type: "sectionEnd", key: "workspace-end" });
		}

		if (olderItems.length > 0) {
			if (breaksFromIncoming === "older")
				virtualRows.push({ type: "regionBreak", key: "older-break" });
			virtualRows.push(
				{ type: "sectionStart", key: "older-start" },
				{ type: "railEdge", key: "older-head", status: "Upstream", edge: "head" },
			);
			for (const item of olderItems) {
				virtualRows.push({
					type: "item",
					key: `older-${itemKey(item)}`,
					item,
					opensRail: false,
					status: "Upstream",
				});
			}
			virtualRows.push(
				{ type: "railEdge", key: "older-tail", status: "Upstream", edge: "tail" },
				{ type: "sectionEnd", key: "older-end" },
			);
		}

		const virtualIndexByAddressKey = new Map<string, number>();
		for (const [index, row] of virtualRows.entries()) {
			if (row.type !== "item" || row.item.type !== "commit") continue;
			virtualIndexByAddressKey.set(addressIdentityKey(upstreamCommitAddress(row.item)), index);
		}

		return { virtualRows, virtualIndexByAddressKey };
	}, [incomingItemCount, isError, isPending, items, olderItems, targetLabel]);

	const selectedAddressKey = selection === null ? undefined : addressIdentityKey(selection);
	const selectedVirtualIndex =
		selectedAddressKey === undefined ? undefined : virtualIndexByAddressKey.get(selectedAddressKey);
	const getVirtualRowKey = useCallback(
		(index: number) => virtualRows[index]?.key ?? index,
		[virtualRows],
	);
	const rangeExtractorWithSelected = useCallback(
		(range: Range) =>
			getRangeExtractorWithIndices(
				range,
				selectedVirtualIndex === undefined ? [] : [selectedVirtualIndex],
			),
		[selectedVirtualIndex],
	);

	// oxlint-disable-next-line react-hooks-js/incompatible-library -- https://github.com/TanStack/virtual/issues/1119#issuecomment-4648268095
	const rowVirtualizer = useVirtualizer({
		directDomUpdates: true,
		directDomUpdatesMode: "transform",
		count: virtualRows.length,
		getScrollElement: () => scrollElementRef.current,
		estimateSize: (index) => {
			switch (virtualRows[index]?.type) {
				case "sectionStart":
					return 6;
				case "sectionEnd":
					return 7;
				case "regionBreak":
					return 9;
				case "railEdge":
					return 8;
				case "runTail":
					return 4;
				case "update":
					return 100;
				default:
					return 40;
			}
		},
		getItemKey: getVirtualRowKey,
		rangeExtractor: rangeExtractorWithSelected,
		// Matches --scroll-gradient-height.
		scrollPaddingStart: 14,
		scrollPaddingEnd: 14,
	});

	const lastRevealedAddressKeyRef = useRef<string>(undefined);
	useLayoutEffect(() => {
		if (
			selectedAddressKey !== undefined &&
			selectedAddressKey !== lastRevealedAddressKeyRef.current &&
			selectedVirtualIndex !== undefined
		)
			rowVirtualizer.scrollToIndex(selectedVirtualIndex, { align: "auto" });

		lastRevealedAddressKeyRef.current = selectedAddressKey;
	}, [rowVirtualizer, selectedAddressKey, selectedVirtualIndex]);

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			{/* Headed like the other tabs: one title over the whole pane, held out
			    of the scroller so it stays put while the listing moves. */}
			<SectionHeaderRow className={styles.header} label="Incoming changes" />

			{/* One graph across three regions: what is coming in, where the
			    workspace's branches sit against it, and the history behind them.
			    Nothing names them — each region's own floor divides it from the
			    next, and the rails carry through. The one wider break falls under
			    everything about what is coming in; see `.regionBreak`. */}
			<div
				tabIndex={0}
				role="tree"
				aria-label="Upstream"
				aria-activedescendant={selection ? treeItemId(selection) : undefined}
				data-focus-scope={"sidebar" satisfies FocusScope}
				className={classes(uiStyles.scroller, styles.list)}
				ref={useMergedRefs(hotkeysRef, retainScrollElement, useAutofocusScope())}
			>
				<div role="none" ref={rowVirtualizer.containerRef} className={styles.virtualContainer}>
					{rowVirtualizer.getVirtualItems().map((virtualRow) => {
						const row = virtualRows[virtualRow.index];
						if (row === undefined) return null;

						return (
							<div
								key={row.key}
								data-index={virtualRow.index}
								ref={rowVirtualizer.measureElement}
								role="none"
								className={classes(
									row.type === "sectionStart" && styles.sectionStart,
									row.type === "sectionEnd" && styles.sectionEnd,
									row.type === "regionBreak" && styles.regionBreak,
									(row.type === "targetHead" ||
										row.type === "message" ||
										row.type === "item" ||
										row.type === "railEdge" ||
										row.type === "runTail") &&
										styles.sectionRow,
									row.type === "railEdge" && styles.railEdgeRow,
								)}
								style={{
									position: "absolute",
									top: 0,
									left: 0,
									width: "100%",
								}}
							>
								{row.type === "targetHead" ? (
									<TargetHeadRow label={row.label} />
								) : row.type === "message" ? (
									<p role="none" className={classes("text-13", styles.message)}>
										{row.message}
									</p>
								) : row.type === "item" ? (
									row.item.type === "commit" ? (
										<TargetCommitRow
											item={row.item}
											positionInSet={
												// oxlint-disable-next-line typescript/no-non-null-assertion
												addressSpace.indexByKey.get(
													addressIdentityKey(upstreamCommitAddress(row.item)),
												)! + 1
											}
											setSize={addressSpace.items.length}
											status={row.status}
										/>
									) : row.item.type === "branch" ? (
										<UpstreamBranchRow
											item={row.item}
											glyph={row.opensRail ? "forkRight" : "joinRight"}
										/>
									) : (
										<SegmentExpanderRow
											projectId={projectId}
											segmentId={row.item.segmentId}
											count={row.item.count}
											expanded={row.item.expanded}
											opensRail={row.opensRail}
										/>
									)
								) : row.type === "railEdge" ? (
									<RailEdge status={row.status} edge={row.edge} />
								) : row.type === "runTail" ? (
									<RunTail />
								) : row.type === "update" ? (
									<UpdateBlock
										incomingCount={incomingCount}
										hasIntegrated={hasIntegrated}
										canUpdate={canUpdate}
										isUpdatePending={isUpdatePending}
										onUpdateWorkspace={onUpdateWorkspace}
									/>
								) : null}
							</div>
						);
					})}
				</div>

				<LoadMoreOlder projectId={projectId} hasOlder={hasOlder} />
			</div>
		</div>
	);
};
