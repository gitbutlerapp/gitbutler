import type { PayloadFor } from "#electron/ipc.ts";
import type { ListedBranch, ListedStack } from "@gitbutler/but-sdk";
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
	if (trimmed.length < MIN_SEARCH_LENGTH) return stacks;

	const fuse = new Fuse(
		stacks.flatMap((stack) => stack.branches),
		{
			keys: ["displayName", "lastAuthor.name", "lastAuthor.email", "review.title"],
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
