import { headInfoQueryOptions, workspaceTargetCommitsQueryOptions } from "#ui/api/queries.ts";
import { usePage } from "#ui/use-cursor.ts";
import { commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { RefInfo, TargetCommit } from "@gitbutler/but-sdk";
import { useQueries } from "@tanstack/react-query";

// Stable empties for the inactive-tab result, so consumers' identities do not
// churn while the tab is hidden.
const noItems: Array<UpstreamListItem> = [];
const emptyNavigationIndex: NavigationIndex<Operand> = { items: [], indexByKey: new Map() };

/**
 * Commit items are cached per target commit, so outline rebuilds (expansion
 * toggles, freshly loaded pages) hand unaffected rows the same item objects
 * and only genuinely new rows render. The query data objects keying the cache
 * stay identical until their query refetches, which is exactly when a row's
 * content may change.
 */
const commitItems = new WeakMap<TargetCommit, UpstreamCommitItem>();
const asItem = (targetCommit: TargetCommit): UpstreamCommitItem => {
	let item = commitItems.get(targetCommit);
	if (item === undefined) {
		item = { type: "commit", ...targetCommit };
		commitItems.set(targetCommit, item);
	}
	return item;
};

/** A target commit as a list item. */
export type UpstreamCommitItem = TargetCommit & { type: "commit" };

/**
 * A workspace branch positioned against the target line, rendered with the
 * same anatomy as the workspace tab's branch rows so the two are recognizably
 * the same thing.
 */
export type UpstreamBranchItem = {
	type: "branch";
	name: string;
	/** Matched against a landed review to place the branch; never displayed. */
	prNumber: number | null;
	integrated: boolean;
	/**
	 * Identifies the stack the branch belongs to, so adjacent segments of one
	 * stack consolidate into a single card like on the workspace tab.
	 */
	stackKey: string;
};

export type UpstreamListItem =
	| UpstreamCommitItem
	| UpstreamBranchItem
	/**
	 * A toggle revealing the shared history between two fork points: target
	 * commits the workspace already has, whose count measures how much older
	 * one stack's base is than the one above it.
	 */
	| { type: "expander"; segmentId: string; count: number; expanded: boolean };

export type UpstreamOutline = {
	items: Array<UpstreamListItem>;
	/**
	 * How many leading items are incoming commits. The list draws its divider
	 * here, so everything after it is the workspace's own branches and the
	 * shared history between them.
	 */
	incomingItemCount: number;
	/** The target's display label, like `origin/main`, or `null` without a target. */
	targetLabel: string | null;
	/**
	 * How many listed commits an update would bring in. Counted from the
	 * first-parent rows once the listing is loaded so the label always agrees
	 * with them; before that, the graph's all-commits count approximates it
	 * for the tab badge.
	 */
	incomingCount: number;
	/** Whether any workspace branch was detected as integrated upstream. */
	hasIntegrated: boolean;
	/** Whether the base listing was clipped before its natural bound. */
	truncated: boolean;
	navigationIndex: NavigationIndex<Operand>;
	/**
	 * The target-commits query's state, so the tab can tell a genuinely empty
	 * result apart from one that has not arrived or failed.
	 */
	isPending: boolean;
	isError: boolean;
};

type WorkspaceStackBranches = {
	/** The commit the stack forks from, when known. */
	base: string | null;
	integrated: Array<UpstreamBranchItem>;
	unintegrated: Array<UpstreamBranchItem>;
};

/** The named workspace branches per stack, split by their integration state. */
const workspaceStackBranches = (headInfo: RefInfo | undefined): Array<WorkspaceStackBranches> =>
	headInfo?.stacks.flatMap((stack): Array<WorkspaceStackBranches> => {
		const stackKey =
			stack.segments.find((segment) => segment.refName !== null)?.refName?.displayName ?? "";
		const branches = stack.segments.flatMap(
			(segment): Array<UpstreamBranchItem> =>
				segment.refName !== null
					? [
							{
								type: "branch",
								name: segment.refName.displayName,
								prNumber: segment.metadata?.review.pullRequest ?? null,
								integrated: segment.pushStatus === "integrated",
								stackKey,
							},
						]
					: [],
		);
		return branches.length > 0
			? [
					{
						base: stack.base,
						integrated: branches.filter((branch) => branch.integrated),
						unintegrated: branches.filter((branch) => !branch.integrated),
					},
				]
			: [];
	}) ?? [];

/**
 * Interleave the target-commit line with the workspace's branches: each
 * unintegrated stack appears directly above the commit it forks from, each
 * integrated branch directly below the commit that landed it. Target commits
 * already in the workspace are not shown; they only order the branches.
 *
 * An integrated branch whose landing commit is unknown (no cached review ties
 * it to one) still landed somewhere in the incoming range — had it landed
 * below the boundary, the previous workspace update would have removed it —
 * so it sits at the bottom of that range, right above the boundary. Fork
 * points missing from the (possibly clipped) list trail at the end.
 *
 * Returns the boundary along with the items: the walk crosses it exactly once,
 * and it cannot be recovered from the items afterwards, because an integrated
 * branch attached to the last incoming commit and one parked at the boundary
 * land in the same place.
 */
const buildItems = (
	commits: Array<UpstreamCommitItem>,
	stacks: Array<WorkspaceStackBranches>,
	expanded: Record<string, true>,
): { items: Array<UpstreamListItem>; incomingItemCount: number } => {
	const stubsByBase = new Map<string, Array<UpstreamBranchItem>>();
	const strandedStubs: Array<UpstreamBranchItem> = [];
	for (const stack of stacks) {
		if (stack.unintegrated.length === 0) continue;
		if (stack.base === null) {
			strandedStubs.push(...stack.unintegrated);
			continue;
		}
		const stubs = stubsByBase.get(stack.base);
		if (stubs !== undefined) stubs.push(...stack.unintegrated);
		else stubsByBase.set(stack.base, [...stack.unintegrated]);
	}

	// Attach each integrated branch to the first commit whose review landed it.
	const allIntegrated = stacks.flatMap((stack) => stack.integrated);
	const matched = new Set<UpstreamBranchItem>();
	const integratedByCommitId = new Map<string, Array<UpstreamBranchItem>>();
	for (const commit of commits) {
		const review = commit.review;
		if (review === null) continue;
		const landed = allIntegrated.filter(
			(branch) =>
				!matched.has(branch) &&
				(branch.prNumber === review.number || branch.name === review.sourceBranch),
		);
		if (landed.length === 0) continue;
		for (const branch of landed) matched.add(branch);
		integratedByCommitId.set(commit.commit.id, landed);
	}

	const unmatchedIntegrated = () => allIntegrated.filter((branch) => !matched.has(branch));

	const items: Array<UpstreamListItem> = [];

	// Commits already in the workspace are not shown outright — the tab is
	// about what's upstream — but each run of them *between* fork points
	// becomes an expandable shared-history segment. The run trailing the
	// deepest fork point has no fork point below it to measure against, so it
	// is dropped rather than flushed: target history simply continues there.
	let gap: Array<UpstreamCommitItem> = [];
	const flushGap = () => {
		const first = gap[0];
		if (first === undefined) return;
		const segmentId = first.commit.id;
		const isExpanded = expanded[segmentId] === true;
		items.push({ type: "expander", segmentId, count: gap.length, expanded: isExpanded });
		if (isExpanded) items.push(...gap);
		gap = [];
	};

	let boundarySeen = false;
	// Everything pushed before the walk crosses into the workspace is the
	// target section, integrated branches attached to those commits included.
	let incomingItemCount = 0;
	for (const commit of commits) {
		const attachedStubs = stubsByBase.get(commit.commit.id);
		if (attachedStubs !== undefined) stubsByBase.delete(commit.commit.id);
		const landed = integratedByCommitId.get(commit.commit.id);

		if (commit.inWorkspace && !boundarySeen) {
			boundarySeen = true;
			incomingItemCount = items.length;
			items.push(...unmatchedIntegrated());
		}
		if (attachedStubs !== undefined || landed !== undefined) flushGap();
		if (attachedStubs !== undefined) items.push(...attachedStubs);
		if (commit.inWorkspace) gap.push(commit);
		else items.push(commit);
		if (landed !== undefined) items.push(...landed);
	}
	// Without a boundary commit in the list the walk never left the target, so
	// everything listed so far belongs to it.
	if (!boundarySeen) incomingItemCount = items.length;

	for (const stranded of stubsByBase.values()) items.push(...stranded);
	items.push(...strandedStubs);
	// Without a boundary commit in the list there is no anchor; keep the
	// integrated branches visible at the end instead of dropping them.
	if (!boundarySeen) items.push(...unmatchedIntegrated());

	return { items, incomingItemCount };
};

/**
 * The upstream tab's combined listing: the target branch's first-parent line
 * annotated with what each commit integrated, interleaved with the workspace's
 * unintegrated stacks at their fork points, plus the matching navigation
 * index. Both the list rendering and the selection resolution in the
 * workspace page consume it, so the two cannot drift apart.
 */
export const useUpstreamOutline = (projectId: string): UpstreamOutline => {
	const active = usePage() === "upstream";
	const expandedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectExpandedUpstreamSegments(state, projectId),
	);
	// The whole derivation lives in `combine` so its result keeps a stable
	// identity: react-query caches it on the query results and the `combine`
	// reference — which itself only changes when a captured input like the
	// expansion map does — so the items and navigation index are not rebuilt
	// on unrelated renders.
	return useQueries({
		queries: [
			{ ...workspaceTargetCommitsQueryOptions(projectId), enabled: active },
			{ ...headInfoQueryOptions(projectId), enabled: active },
		],
		combine: ([targetResult, headInfoResult]): UpstreamOutline => {
			const targetPage = targetResult.data;
			const targetCommits = targetPage?.commits ?? [];
			const headInfo = headInfoResult.data;

			const incomingCount = targetPage
				? targetCommits.filter((commit) => !commit.inWorkspace).length
				: (headInfo?.target?.commitsAhead ?? 0);
			const targetLabel = headInfo?.target
				? `${headInfo.target.remoteTrackingRef.remoteName}/${headInfo.target.remoteTrackingRef.displayName}`
				: null;
			const truncated = targetPage?.hasMore === true;
			const isPending = targetResult.isPending || headInfoResult.isPending;
			const isError = targetResult.isError || headInfoResult.isError;

			// While another tab is shown, only the tab badge consumes this
			// outline, but headInfo refetches on every workspace mutation —
			// skip the item and navigation-index derivation nobody would see.
			if (!active) {
				return {
					items: noItems,
					incomingItemCount: 0,
					targetLabel,
					incomingCount,
					hasIntegrated: false,
					truncated,
					navigationIndex: emptyNavigationIndex,
					isPending,
					isError,
				};
			}

			const stacks = workspaceStackBranches(headInfo);
			const commits = targetCommits.map(asItem);
			const { items, incomingItemCount } = buildItems(commits, stacks, expandedSegments);

			const navigationItems = items.flatMap((item): Array<Operand> => {
				switch (item.type) {
					case "commit":
						// Upstream commits often carry no change-id; the commit id is
						// the fallback identity.
						return [
							commitOperand({
								commitId: item.commit.id,
								changeId: item.commit.changeId ?? item.commit.id,
							}),
						];
					// Branch rows carry only their name and are not selectable, so
					// they stay out of the navigation index.
					case "branch":
					case "expander":
						return [];
					default:
						return item satisfies never;
				}
			});

			return {
				items,
				incomingItemCount,
				targetLabel,
				incomingCount,
				hasIntegrated: stacks.some((stack) => stack.integrated.length > 0),
				truncated,
				navigationIndex: {
					items: navigationItems,
					indexByKey: buildIndexByKey(navigationItems, operandIdentityKey),
				},
				isPending,
				isError,
			};
		},
	});
};
