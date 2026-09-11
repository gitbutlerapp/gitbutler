import type { PayloadFor } from "#electron/ipc.ts";
import type { ListedBranch, ListedStack, RemoteTrackingReference } from "@gitbutler/but-sdk";
import Fuse from "fuse.js";

/**
 * Whether the branch holds no commits of its own — it was just created, or
 * everything it held is now in the branch below it or in the target.
 *
 * This is `commitCount`, the branch's own contribution, and not
 * `commitsAheadOfTarget`, which for a stacked branch also counts the commits
 * of the branches below it. A `null` count (shallow clone, clipped traversal)
 * is unknown rather than empty.
 */
export const branchIsEmpty = (branch: ListedBranch): boolean => branch.commitCount === 0;

/** A remote-tracking ref as shown: `origin/main`. */
export const remoteTrackingLabel = (ref: RemoteTrackingReference): string =>
	`${ref.remoteName}/${ref.displayName}`;

/**
 * The commits the branch contributes itself, taken from a branch-details commit
 * list.
 *
 * Branch details walk all the way down to the target, so for a stacked branch
 * the list also holds the commits of the branches below it. The list is
 * tip-first and `commitCount` is this branch's own contribution, so the head of
 * the list is exactly that. An unknown count keeps everything.
 */
export const branchOwnCommits = <T>(branch: ListedBranch, commits: Array<T>): Array<T> =>
	commits.slice(0, branch.commitCount ?? undefined);

export type BranchFilters = {
	/** Include branches holding no commits of their own. */
	showEmpty: boolean;
	/** Drop branches that exist only on a remote. */
	onlyLocal: boolean;
	/** Keep only stacks that still have more than one branch. */
	onlyStacks: boolean;
};

/**
 * How many filter options are switched on. Every option is off at rest, so
 * this is what the header's badge shows and what "any filter active" means.
 */
export const activeBranchFilterCount = (filters: BranchFilters): number =>
	Object.values(filters).filter(Boolean).length;

/**
 * The stacks from the branch listing that are not applied to the workspace,
 * keeping the listing's most-recent-first order. `showEmpty`/`onlyLocal` prune
 * branches within each stack; `onlyStacks` then keeps just the multi-branch
 * stacks. Either way, stacks left with nothing to show are dropped.
 */
export const unappliedStacks = (
	stacks: Array<ListedStack>,
	{ showEmpty, onlyLocal, onlyStacks }: BranchFilters,
): Array<ListedStack> =>
	stacks
		.filter((stack) => stack.status === "unapplied" || stack.status === "standalone")
		.map((stack) => ({
			...stack,
			branches: stack.branches.filter(
				(branch) => (showEmpty || !branchIsEmpty(branch)) && (!onlyLocal || branch.hasLocal),
			),
		}))
		.filter((stack) => stack.branches.length > (onlyStacks ? 1 : 0));

/** One-character fuzzy queries match too much to be useful; treated as no filter. */
const MIN_SEARCH_LENGTH = 2;

/**
 * The stacks with any branch fuzzily matching `query`, keeping matched stacks
 * whole. A query shorter than {@link MIN_SEARCH_LENGTH} filters nothing. Runs on
 * whatever the empty-branch filter left, so the two compose.
 */
export const searchStacks = (stacks: Array<ListedStack>, query: string): Array<ListedStack> => {
	const trimmed = query.trim();
	const reviewNumber = /^([#!])(\d+)$/.exec(trimmed);
	if (reviewNumber) {
		const [, symbol, number] = reviewNumber;
		return stacks.filter((stack) =>
			stack.branches.some(
				({ review }) =>
					review !== null && String(review.number) === number && review.unitSymbol === symbol,
			),
		);
	}
	if (trimmed.length < MIN_SEARCH_LENGTH) return stacks;

	const fuse = new Fuse(
		stacks.flatMap((stack) => stack.branches),
		{
			keys: [
				"displayName",
				"lastAuthor.name",
				"lastAuthor.email",
				"review.title",
				"review.number",
				"review.labels.name",
				"review.author.login",
				"review.author.name",
			],
			// Desktop's branch-search calibration: forgiving of typos without
			// returning half the list; ignoreLocation matches anywhere in the string.
			threshold: 0.3,
			ignoreLocation: true,
		},
	);
	// fuse returns the branch objects we passed in, so match by identity.
	const matched = new Set(fuse.search(trimmed).map((result) => result.item));

	return stacks.filter((stack) => stack.branches.some((branch) => matched.has(branch)));
};

/**
 * Splits a full ref name into the branch name and remote as expected by the
 * branch details API.
 */
// https://linear.app/gitbutler/issue/GB-1226/unify-branch-identifiers
export const branchDetailsParams = (
	refName: string,
): Pick<PayloadFor<"branchDetails">, "branchName" | "remote"> => {
	const remoteMatch = /^refs\/remotes\/([^/]+)\/(.+)$/.exec(refName);
	const remote = remoteMatch?.[1];
	const branchName = remoteMatch?.[2];

	return remote !== undefined && branchName !== undefined
		? { branchName, remote }
		: { branchName: refName.replace(/^refs\/heads\//, ""), remote: null };
};

export type BranchGrouping = "state" | "author" | "recent";
export type BranchListGroup = { key: string; label: string; count: number; collapsed: boolean };
type GroupedBranch = { branch: ListedBranch; isTopBranch: boolean; isStacked: boolean };
type BranchListSection = {
	key: string;
	group?: BranchListGroup;
	branches: Array<GroupedBranch>;
	grouped: boolean;
};

export const branchListSections = (
	stacks: Array<ListedStack>,
	grouping: BranchGrouping,
	collapsedGroups: Record<string, boolean>,
): Array<BranchListSection> => {
	const entries = stacks.map((stack) =>
		stack.branches.map((branch, index) => ({
			branch,
			isTopBranch: index === 0,
			isStacked: stack.branches.length > 1,
		})),
	);
	if (grouping === "recent") {
		return entries.flatMap((branches) => {
			const first = branches[0];
			return first ? [{ key: first.branch.refName.full, branches, grouped: false }] : [];
		});
	}
	const labels: Record<string, string> = {
		open: "Open",
		draft: "Draft",
		none: "No pull request",
		merged: "Merged",
		closed: "Closed",
	};
	const groups = new Map<string, Array<GroupedBranch>>();
	for (const stack of entries) {
		for (const entry of stack) {
			const value =
				grouping === "state"
					? (entry.branch.reviewStatus ?? "none")
					: (entry.branch.review?.author?.login ??
						entry.branch.lastAuthor?.name ??
						"Unknown author");
			const members = groups.get(value);
			if (members) members.push(entry);
			else groups.set(value, [entry]);
		}
	}
	const keys =
		grouping === "state"
			? Object.keys(labels).filter((key) => groups.has(key))
			: Array.from(groups.keys()).sort((a, b) => a.localeCompare(b));
	return keys.flatMap((value): Array<BranchListSection> => {
		const branches = groups.get(value);
		if (!branches) return [];
		const key = `${grouping}:${value}`;
		const collapsed =
			collapsedGroups[key] ?? (grouping === "state" && (value === "merged" || value === "closed"));
		return [
			{
				key,
				group: {
					key,
					label: grouping === "state" ? (labels[value] ?? value) : value,
					count: branches.length,
					collapsed,
				},
				branches: [],
				grouped: true,
			},
			...(collapsed
				? []
				: branches.map((entry) => ({
						key: entry.branch.refName.full,
						branches: [entry],
						grouped: true,
					}))),
		];
	});
};
