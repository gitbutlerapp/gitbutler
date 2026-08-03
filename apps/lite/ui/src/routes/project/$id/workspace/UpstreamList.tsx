import rowStyles from "./Row.module.css";
import { Scroller } from "#ui/components/Scroller.tsx";
import { forgeInfoOptions } from "#ui/api/queries.ts";
import { commitTitle } from "#ui/commit.ts";
import { Badge } from "#ui/components/Badge.tsx";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { GraphSegment } from "#ui/components/GraphSegment.tsx";
import { Icon } from "#ui/components/Icon.tsx";
import { branchOperand, commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { prForgeUrl } from "#ui/pr.ts";
import { projectSlice } from "#ui/projects/state.ts";
import {
	useAutofocusSelectionScope,
	useNavigationIndexHotkeys,
	type SelectionScope,
} from "#ui/selection-scopes.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { formatRelativeTime } from "#ui/time.ts";
import { useMergedRefs } from "@base-ui/utils/useMergedRefs";
import { useQuery } from "@tanstack/react-query";
import {
	type ComponentProps,
	type FC,
	type MouseEvent,
	useEffect,
	useId,
	useRef,
	useState,
} from "react";
import { Row, RowLabel, RowLabelContainer, RowLabelFooter } from "./Row.tsx";
import {
	selectionOutOfSync,
	treeItemId,
	useIsSelected as useIsSelectedInList,
} from "./Row-utils.ts";
import {
	INCOMING_SEGMENT_ID,
	useOlderTargetCommits,
	type UpstreamBranchItem,
	type UpstreamCommitItem,
	type UpstreamListItem,
	type UpstreamOutline,
} from "./useUpstreamOutline.ts";
import styles from "./UpstreamList.module.css";

const useIsSelected = (projectId: string, operand: Operand): boolean =>
	useIsSelectedInList(projectId, operand, projectSlice.selectors.selectPrimaryUpstreamSelection);

const TargetCommitRow: FC<{ projectId: string; item: UpstreamCommitItem }> = ({
	projectId,
	item,
}) => {
	const dispatch = useAppDispatch();
	const { commit, review, inWorkspace } = item;
	const operand = commitOperand({ commitId: commit.id, changeId: commit.changeId ?? commit.id });
	const isSelected = useIsSelected(projectId, operand);
	const title = commitTitle(commit.message);
	const [now] = useState(() => Date.now());

	const authored = [commit.author.name, formatRelativeTime(commit.committedAt, now)]
		.filter((part) => part !== "")
		.join(" ");

	const openReviewInBrowser = async (evt: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt.preventDefault();

		if (review) await window.lite.openInWebBrowser(review.htmlUrl);
	};

	return (
		<Row
			id={treeItemId(operand)}
			role="treeitem"
			aria-label={title ?? "(no message)"}
			aria-selected={isSelected}
			isSelected={isSelected}
			onSelect={() =>
				dispatch(projectSlice.actions.selectUpstream({ projectId, selection: operand }))
			}
		>
			<GraphSegment glyph="commit" status={inWorkspace ? "Integrated" : "LocalAndRemote"} />
			<div className={styles.label}>
				<RowLabelContainer>
					<RowLabel singleLine className={inWorkspace ? rowStyles.fadedText : undefined}>
						{title === undefined ? (
							<span className={rowStyles.fadedText}>(no message)</span>
						) : (
							title
						)}
					</RowLabel>
				</RowLabelContainer>
				{(authored !== "" || review !== null) && (
					<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
						{authored !== "" && (
							<span
								className={classes(rowStyles.fadedText, styles.labelMetaItem)}
								title={commit.author.email}
							>
								{authored}
							</span>
						)}

						{review !== null && (
							<a
								href={review.htmlUrl}
								title={review.title}
								onClick={(evt) => void openReviewInBrowser(evt)}
								className={classes(rowStyles.fadedText, styles.labelMetaItem)}
							>
								<Icon name="pr" />
								{review.unitSymbol}
								{review.number}
							</a>
						)}
					</RowLabelFooter>
				)}
			</div>
		</Row>
	);
};

/**
 * A workspace branch positioned against the target line, with the same row
 * anatomy as the workspace tab's branch rows: bold name, meta footer, and PR
 * link. Branch rows deliberately stay off the commit line — the state pill
 * alone carries their relation to the upstream.
 */
const UpstreamBranchRow: FC<{ projectId: string; item: UpstreamBranchItem }> = ({
	projectId,
	item,
}) => {
	const dispatch = useAppDispatch();
	const operand = branchOperand({ branchRef: item.refBytes });
	const isSelected = useIsSelected(projectId, operand);

	const { data: forgeInfo } = useQuery(forgeInfoOptions(projectId));
	const mforgeUrl =
		item.prNumber !== null ? forgeInfo && prForgeUrl(item.prNumber, forgeInfo) : null;

	const openPRInBrowser = async (evt: MouseEvent<HTMLAnchorElement>): Promise<void> => {
		evt.preventDefault();

		if (mforgeUrl != null) await window.lite.openInWebBrowser(mforgeUrl);
	};

	return (
		<Row
			id={treeItemId(operand)}
			role="treeitem"
			aria-label={item.name}
			aria-selected={isSelected}
			isSelected={isSelected}
			onSelect={() =>
				dispatch(projectSlice.actions.selectUpstream({ projectId, selection: operand }))
			}
		>
			<GraphSegment glyph="space" status="LocalOnly" />
			<div className={styles.label}>
				<RowLabelContainer>
					<RowLabel heading singleLine title={item.name}>
						{item.name}
					</RowLabel>
				</RowLabelContainer>
				{/* Always rendered so integrated and unintegrated rows share the
				    same height regardless of what the footer carries. */}
				<RowLabelFooter className={classes("text-13", styles.labelMeta)}>
					{item.commitCount > 0 && (
						<span className={classes(rowStyles.fadedText, styles.labelMetaItem)}>
							<Icon name="commit" />
							{item.commitCount}
						</span>
					)}

					{mforgeUrl != null && (
						<a
							href={mforgeUrl}
							onClick={(evt) => void openPRInBrowser(evt)}
							className={classes(rowStyles.fadedText, styles.labelMetaItem)}
						>
							<Icon name="pr" size={14} />
							PR
						</a>
					)}
				</RowLabelFooter>
			</div>
			{item.integrated ? (
				<Badge
					variant="integrated"
					className={styles.statePill}
					title="Merged into the target branch; updating the workspace cleans it up."
				>
					Integrated
				</Badge>
			) : (
				<Badge
					variant="lightGray"
					className={styles.statePill}
					title="Branching off here; not part of the target branch yet."
				>
					In workspace
				</Badge>
			)}
		</Row>
	);
};

type CommitRunItem = Exclude<UpstreamListItem, UpstreamBranchItem>;

type ItemGroup =
	/** Adjacent segments of one stack share a card, like on the workspace tab. */
	| { branches: Array<UpstreamBranchItem>; rows?: undefined }
	| { branches?: undefined; rows: Array<CommitRunItem> };

/**
 * Split the flat listing into stack cards and the bare commit runs flowing
 * between them, so the workspace's branches read as their own objects rather
 * than part of the target history.
 */
const groupItems = (items: Array<UpstreamListItem>): Array<ItemGroup> => {
	const groups: Array<ItemGroup> = [];
	for (const item of items) {
		const last = groups.at(-1);
		if (item.type === "branch") {
			if (last?.branches !== undefined && last.branches[0]?.stackKey === item.stackKey)
				last.branches.push(item);
			else groups.push({ branches: [item] });
			continue;
		}
		if (last?.rows !== undefined) last.rows.push(item);
		else groups.push({ rows: [item] });
	}
	return groups;
};

const groupKey = (rows: Array<CommitRunItem>): string => {
	const first = rows[0];
	if (first === undefined) return "empty";
	switch (first.type) {
		case "commit":
			return first.commit.id;
		case "expander":
			return `expander-${first.segmentId}`;
		case "more":
			return "more";
		default:
			return first satisfies never;
	}
};

const SegmentExpanderRow: FC<{
	projectId: string;
	segmentId: string;
	count: number | null;
	expanded: boolean;
}> = ({ projectId, segmentId, count, expanded }) => {
	const dispatch = useAppDispatch();
	const incoming = segmentId === INCOMING_SEGMENT_ID;

	return (
		<button
			type="button"
			// The tree's active-descendant navigation skips expanders, but as
			// direct children of the tree they still need the treeitem role for
			// the structure to stay valid.
			role="treeitem"
			aria-expanded={expanded}
			className={classes("text-13", rowStyles.fadedText, styles.expanderRow)}
			title={
				incoming
					? "The commits an update would bring into the workspace."
					: count !== null
						? "Shared target commits between these fork points."
						: "Target history below this fork point."
			}
			onClick={() =>
				dispatch(
					incoming
						? projectSlice.actions.toggleUpstreamIncoming({ projectId })
						: projectSlice.actions.toggleUpstreamSegment({ projectId, segmentId }),
				)
			}
		>
			⋮{" "}
			{expanded
				? "hide"
				: count !== null
					? `${count} ${count === 1 ? "commit" : "commits"}`
					: "older commits…"}
		</button>
	);
};

const OlderCommitsMoreRow: FC<{ projectId: string }> = ({ projectId }) => {
	// Shares the pages the outline hook merges into the list; this instance
	// only drives fetching. Only rendered while the older segment is expanded.
	const { fetchNextPage, isFetching, isError } = useOlderTargetCommits(projectId, true);

	return (
		<button
			type="button"
			// Same as the segment expanders: a treeitem in structure, though
			// active-descendant navigation skips it.
			role="treeitem"
			className={classes("text-13", rowStyles.fadedText, styles.expanderRow)}
			disabled={isFetching}
			onClick={() => void fetchNextPage()}
		>
			{isFetching ? (
				<Icon name="spinner" />
			) : isError ? (
				"⋮ couldn't load older commits — retry"
			) : (
				"⋮ show more…"
			)}
		</button>
	);
};

export const UpstreamList: FC<
	{
		projectId: string;
		outline: UpstreamOutline;
		canUpdateWorkspace: boolean;
		isUpdatePending: boolean;
		onUpdateWorkspace: () => void;
	} & ComponentProps<"div">
> = ({
	projectId,
	outline,
	canUpdateWorkspace,
	isUpdatePending,
	onUpdateWorkspace,
	...restProps
}) => {
	const dispatch = useAppDispatch();
	// Derived once in WorkspacePage and passed down, so the rendered list and the
	// navigation index that resolves selection are the same object.
	const { items, targetLabel, incomingCount, hasIntegrated, navigationIndex, isPending, isError } =
		outline;

	const selection = useAppSelector((state) =>
		projectSlice.selectors.selectSelectionUpstream(state, projectId, navigationIndex),
	);
	const storedSelection = useAppSelector((state) =>
		projectSlice.selectors.selectPrimaryUpstreamSelection(state, projectId),
	);

	const outOfSyncSelection = selectionOutOfSync(selection, storedSelection);
	useEffect(() => {
		if (outOfSyncSelection !== null)
			dispatch(projectSlice.actions.selectUpstream({ projectId, selection: outOfSyncSelection }));
	}, [dispatch, outOfSyncSelection, projectId]);

	const headingId = useId();
	const hotkeysRef = useRef<HTMLDivElement>(null);

	useNavigationIndexHotkeys({
		navigationIndex,
		projectId,
		group: "Outline",
		select: (newItem) =>
			dispatch(projectSlice.actions.selectUpstream({ projectId, selection: newItem })),
		selection,
		ref: hotkeysRef,
		getKey: operandIdentityKey,
	});

	const hasIncoming = incomingCount > 0;
	const canUpdate = canUpdateWorkspace && (hasIncoming || hasIntegrated);

	return (
		<div {...restProps} className={classes(restProps.className, styles.container)}>
			<Scroller className={styles.listArea} viewportClassName={styles.list}>
				<h4 id={headingId} className={classes("text-13", styles.heading)}>
					<span>
						Incoming changes
						{targetLabel !== null && (
							<span className={rowStyles.fadedText}> from {targetLabel}</span>
						)}
					</span>
					{hasIncoming && <Badge variant="fillGray">{incomingCount}</Badge>}
				</h4>

				{isError && (
					<p className={classes("text-13", styles.heading)}>Unable to load incoming commits.</p>
				)}

				{!isError && isPending && items.length === 0 && (
					<p className={classes("text-13", styles.heading)}>Loading incoming commits…</p>
				)}

				{!isError && !isPending && targetLabel === null && (
					<p className={classes("text-13", styles.heading)}>
						No target branch is configured for this project.
					</p>
				)}

				{!isError && !isPending && targetLabel !== null && !hasIncoming && (
					<p className={classes("text-13", styles.heading)}>Your workspace is up to date.</p>
				)}

				<div
					tabIndex={0}
					role="tree"
					aria-labelledby={headingId}
					aria-activedescendant={selection ? treeItemId(selection) : undefined}
					data-selection-scope={"outline" satisfies SelectionScope}
					className={styles.tree}
					onFocus={() =>
						dispatch(projectSlice.actions.setDetailsSelectionScope({ projectId, scope: "outline" }))
					}
					ref={useMergedRefs(hotkeysRef, useAutofocusSelectionScope())}
				>
					{groupItems(items).map((group) =>
						group.branches !== undefined ? (
							// The card surface matches the workspace tab's stack cards: the
							// workspace's own stacks are carded objects, target history
							// flows bare around them.
							// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A stack is an ARIA group of tree items.
							<div key={group.branches[0]?.name ?? "stack"} role="group" className={styles.card}>
								{group.branches.map((branch) => (
									<UpstreamBranchRow key={branch.name} projectId={projectId} item={branch} />
								))}
							</div>
						) : (
							// oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- A commit run is an ARIA group of tree items.
							<div key={groupKey(group.rows)} role="group" className={styles.commitRun}>
								{group.rows.map((item) =>
									item.type === "commit" ? (
										<TargetCommitRow key={item.commit.id} projectId={projectId} item={item} />
									) : item.type === "expander" ? (
										<SegmentExpanderRow
											key={`expander-${item.segmentId}`}
											projectId={projectId}
											segmentId={item.segmentId}
											count={item.count}
											expanded={item.expanded}
										/>
									) : (
										<OlderCommitsMoreRow key="more" projectId={projectId} />
									),
								)}
							</div>
						),
					)}
				</div>
				{outline.truncated && (
					<p className={classes("text-13", rowStyles.fadedText, styles.heading)}>
						Only the most recent target history is shown.
					</p>
				)}
			</Scroller>

			<footer className={styles.footer}>
				{hasIncoming ? (
					<p className={classes("text-13", styles.footerText)}>
						Your workspace is {incomingCount} {incomingCount === 1 ? "commit" : "commits"} behind
						the upstream. Update to rebase all stacks at once.
					</p>
				) : (
					hasIntegrated && (
						<p className={classes("text-13", styles.footerText)}>
							Integrated branches can be cleaned up by updating the workspace.
						</p>
					)
				)}
				<button
					type="button"
					className={getButtonClassName({ variant: "pop" })}
					disabled={!canUpdate}
					onClick={onUpdateWorkspace}
				>
					{isUpdatePending ? "Updating…" : "Update workspace"}
				</button>
			</footer>
		</div>
	);
};
