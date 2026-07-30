import {
	headInfoQueryOptions,
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { branchOperand, commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { RefInfo, TargetCommit } from "@gitbutler/but-sdk";
import { useInfiniteQuery, useQueries, useQuery } from "@tanstack/react-query";

/**
 * The expansion key of the shared-history segment below the deepest fork
 * point, whose contents page through older target history.
 */
const OLDER_SEGMENT_ID = "older";

/**
 * The expansion key of the incoming-commits run. Unlike the shared-history
 * segments it shows by default, and hiding it keeps the integrated branch
 * rows attached to it visible.
 */
export const INCOMING_SEGMENT_ID = "incoming";

/** A target commit annotated with the workspace boundary. */
export type UpstreamCommitItem = TargetCommit & {
	type: "commit";
	/** Whether this is the first commit already in the workspace, i.e. the boundary. */
	workspaceBoundary: boolean;
};

/**
 * A workspace branch positioned against the target line, rendered with the
 * same anatomy as the workspace tab's branch rows so the two are recognizably
 * the same thing.
 */
export type UpstreamBranchItem = {
	type: "branch";
	name: string;
	refBytes: Array<number>;
	prNumber: number | null;
	commitCount: number;
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
	 * A toggle revealing a shared-history segment: the target commits between
	 * two fork points (`count` known), or — below the deepest fork point —
	 * older target history paged on demand (`count` null).
	 */
	| { type: "expander"; segmentId: string; count: number | null; expanded: boolean }
	/** Fetches the next page of older target history. */
	| { type: "more" };

export type UpstreamOutline = {
	items: Array<UpstreamListItem>;
	/** The target's display label, like `origin/main`, or `null` without a target. */
	targetLabel: string | null;
	/** How many listed commits are not yet in the workspace. */
	incomingCount: number;
	/** Whether any workspace branch was detected as integrated upstream. */
	hasIntegrated: boolean;
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
								refBytes: segment.refName.fullNameBytes,
								prNumber: segment.metadata?.review.pullRequest ?? null,
								commitCount: segment.commits.length,
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
 */
const buildItems = (
	commits: Array<UpstreamCommitItem>,
	stacks: Array<WorkspaceStackBranches>,
	expanded: Record<string, true>,
	incomingHidden: boolean,
	older: { commits: Array<UpstreamCommitItem>; showMore: boolean },
): Array<UpstreamListItem> => {
	const stubsByBase = new Map<string, Array<Array<UpstreamBranchItem>>>();
	for (const stack of stacks) {
		if (stack.unintegrated.length === 0) continue;
		if (stack.base === null) {
			stubsByBase.set("", [...(stubsByBase.get("") ?? []), stack.unintegrated]);
			continue;
		}
		stubsByBase.set(stack.base, [...(stubsByBase.get(stack.base) ?? []), stack.unintegrated]);
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
	// about what's upstream — but each run of them between fork points becomes
	// an expandable shared-history segment.
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

	const incomingCount = commits.filter((commit) => !commit.inWorkspace).length;
	let boundaryEmitted = false;
	let incomingExpanderEmitted = false;
	for (const commit of commits) {
		const attachedStubs = stubsByBase.get(commit.commit.id);
		if (attachedStubs) stubsByBase.delete(commit.commit.id);
		const landed = integratedByCommitId.get(commit.commit.id);

		if (commit.workspaceBoundary) {
			items.push(...unmatchedIntegrated());
			boundaryEmitted = true;
		}
		if (attachedStubs !== undefined || landed !== undefined) flushGap();
		for (const stub of attachedStubs ?? []) items.push(...stub);
		if (!commit.inWorkspace) {
			if (!incomingExpanderEmitted) {
				items.push({
					type: "expander",
					segmentId: INCOMING_SEGMENT_ID,
					count: incomingCount,
					expanded: !incomingHidden,
				});
				incomingExpanderEmitted = true;
			}
			// Hiding the incoming commits keeps the integrated branch rows they
			// landed visible below.
			if (!incomingHidden) items.push(commit);
		} else {
			gap.push(commit);
		}
		items.push(...(landed ?? []));
	}
	for (const stranded of stubsByBase.values()) for (const stub of stranded) items.push(...stub);
	// Without a boundary commit in the list there is no anchor; keep the
	// integrated branches visible at the end instead of dropping them.
	if (!boundaryEmitted) items.push(...unmatchedIntegrated());

	// The trailing shared history below the deepest fork point continues into
	// older target commits, paged on demand through the same expander.
	if (gap.length > 0 || commits.length > 0) {
		const olderExpanded = expanded[OLDER_SEGMENT_ID] === true;
		items.push({
			type: "expander",
			segmentId: OLDER_SEGMENT_ID,
			count: null,
			expanded: olderExpanded,
		});
		if (olderExpanded) {
			items.push(...gap);
			items.push(...older.commits);
			if (older.showMore) items.push({ type: "more" });
		}
	}

	return items;
};

/**
 * The upstream tab's combined listing: the target branch's first-parent line
 * annotated with what each commit integrated, interleaved with the workspace's
 * unintegrated stacks at their fork points, plus the matching navigation
 * index. Both the list rendering and the selection resolution in the
 * workspace page consume it, so the two cannot drift apart.
 */
export const useUpstreamOutline = (projectId: string): UpstreamOutline => {
	const active = useAppSelector(
		(state) => projectSlice.selectors.selectOutlineTab(state, projectId) === "upstream",
	);
	const expandedSegments = useAppSelector((state) =>
		projectSlice.selectors.selectExpandedUpstreamSegments(state, projectId),
	);
	const incomingHidden = useAppSelector((state) =>
		projectSlice.selectors.selectUpstreamIncomingHidden(state, projectId),
	);

	// Older target history pages continue below the last commit of the base
	// listing; the cursor comes from the same cached response.
	const { data: olderFrom = null } = useQuery({
		...workspaceTargetCommitsQueryOptions(projectId),
		enabled: active,
		select: (commits) => commits.at(-1)?.commit.id ?? null,
	});
	const olderExpanded = expandedSegments[OLDER_SEGMENT_ID] === true;
	const olderQuery = useInfiniteQuery({
		...olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom ?? ""),
		enabled: active && olderExpanded && olderFrom !== null,
	});
	const olderPages = olderQuery.data;
	const olderShowMore = olderExpanded && (olderPages === undefined || olderQuery.hasNextPage);

	// The whole derivation lives in `combine` so its result keeps a stable
	// identity: react-query caches it on the query results and the `combine`
	// reference — which itself only changes when a captured input like the
	// expansion map or a fetched page does — so the items and navigation index
	// are not rebuilt on unrelated renders.
	return useQueries({
		queries: [
			{ ...workspaceTargetCommitsQueryOptions(projectId), enabled: active },
			{ ...headInfoQueryOptions(projectId), enabled: active },
		],
		combine: ([targetResult, headInfoResult]): UpstreamOutline => {
			const targetCommits = targetResult.data ?? [];
			const headInfo = headInfoResult.data;
			const stacks = workspaceStackBranches(headInfo);

			let boundarySeen = false;
			const commits = targetCommits.map((targetCommit): UpstreamCommitItem => {
				const workspaceBoundary = targetCommit.inWorkspace && !boundarySeen;
				boundarySeen ||= targetCommit.inWorkspace;
				return { type: "commit", ...targetCommit, workspaceBoundary };
			});

			const olderCommits =
				olderPages?.pages.flat().map(
					(targetCommit): UpstreamCommitItem => ({
						type: "commit",
						...targetCommit,
						workspaceBoundary: false,
					}),
				) ?? [];
			const items = buildItems(commits, stacks, expandedSegments, incomingHidden, {
				commits: olderCommits,
				showMore: olderShowMore,
			});

			const navigationItems = items.flatMap((item): Array<Operand> => {
				switch (item.type) {
					case "commit":
						// The backend synthesizes a change-id when the commit has none,
						// but the transport type stays nullable; the commit id is the
						// same fallback identity.
						return [
							commitOperand({
								commitId: item.commit.id,
								changeId: item.commit.changeId ?? item.commit.id,
							}),
						];
					case "branch":
						return [branchOperand({ branchRef: item.refBytes })];
					case "expander":
					case "more":
						return [];
					default:
						return item satisfies never;
				}
			});

			return {
				items,
				targetLabel: headInfo?.target
					? `${headInfo.target.remoteTrackingRef.remoteName}/${headInfo.target.remoteTrackingRef.displayName}`
					: null,
				incomingCount: commits.filter((commit) => !commit.inWorkspace).length,
				hasIntegrated: stacks.some((stack) => stack.integrated.length > 0),
				navigationIndex: {
					items: navigationItems,
					indexByKey: buildIndexByKey(navigationItems, operandIdentityKey),
				},
				isPending: targetResult.isPending,
				isError: targetResult.isError,
			};
		},
	});
};
