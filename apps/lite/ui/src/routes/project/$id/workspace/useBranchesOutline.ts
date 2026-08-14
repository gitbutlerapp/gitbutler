import { branchDetailsQueryOptions, branchListQueryOptions } from "#ui/api/queries.ts";
import { usePage } from "#ui/use-cursor.ts";
import { encodeBytes } from "#ui/api/bytes.ts";
import {
	branchDetailsParams,
	branchIsEmpty,
	branchOwnCommits,
	searchStacks,
	unappliedStacks,
} from "#ui/branch.ts";
import { branchOperand, commitOperand, operandIdentityKey, type Operand } from "#ui/operands.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { buildIndexByKey, type NavigationIndex } from "#ui/workspace/navigation-index.ts";
import type { ListedStack } from "@gitbutler/but-sdk";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useDeferredValue } from "react";

export type BranchesOutline = {
	stacks: Array<ListedStack>;
	navigationIndex: NavigationIndex<Operand>;
	/**
	 * The listing query's state, so the tab can tell a genuinely empty result
	 * apart from one that has not arrived or failed.
	 */
	isPending: boolean;
	isError: boolean;
};

type OutlineContent = Pick<BranchesOutline, "stacks" | "navigationIndex">;

const emptyContent: OutlineContent = {
	stacks: [],
	navigationIndex: { items: [], indexByKey: new Map() },
};

/**
 * The branches tab's visible stacks and the matching navigation index.
 *
 * This is the single source of truth for what the tab shows: both the list
 * rendering and the selection resolution in the workspace page consume it, so
 * filtering and fold state cannot drift between the two.
 */
export const useBranchesOutline = (projectId: string): BranchesOutline => {
	const active = usePage() === "branches";
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	// Deferred so the fuzzy filter runs at low priority and typing stays
	// responsive; the input itself is controlled by the non-deferred value.
	const search = useDeferredValue(
		useAppSelector((state) => projectSlice.selectors.selectBranchSearch(state, projectId)),
	);
	const unfoldedBranches = useAppSelector((state) =>
		projectSlice.selectors.selectUnfoldedBranches(state, projectId),
	);

	const unfoldedBranchRefs = active ? Object.keys(unfoldedBranches) : [];
	const commitsByRef = useQueries({
		queries: unfoldedBranchRefs.map((refName) =>
			branchDetailsQueryOptions({ projectId, ...branchDetailsParams(refName) }),
		),
		combine: (results) =>
			new Map(
				unfoldedBranchRefs.map((refName, index) => [
					refName,
					results[index]?.data?.commits.map((commit) =>
						commitOperand({ commitId: commit.id, changeId: commit.changeId }),
					) ?? [],
				]),
			),
	});

	// The whole derivation lives in `select` so its result keeps a stable
	// identity: react-query caches it on the query data and the `select`
	// reference, and React Compiler memoizes this inline closure by its captured
	// inputs — so the closure, and thus the cached result, only changes when an
	// input like `search` or `showEmpty` does. Deriving in render instead would
	// rebuild the navigation index every pass and re-render every row that reads
	// it through context.
	const {
		data: content = emptyContent,
		isPending,
		isError,
	} = useQuery({
		...branchListQueryOptions(projectId),
		enabled: active,
		select: (listedStacks): OutlineContent => {
			const stacks = searchStacks(unappliedStacks(listedStacks, filters), search);
			const items = stacks.flatMap((stack) =>
				stack.branches.flatMap(
					(branch): Array<Operand> => [
						branchOperand({ branchRef: encodeBytes(branch.refName.full) }),
						// Matches what BranchesList renders: a branch with no commits of
						// its own cannot be unfolded, and an unfolded one shows only the
						// commits it contributes itself.
						...(unfoldedBranches[branch.refName.full] && !branchIsEmpty(branch)
							? branchOwnCommits(branch, commitsByRef.get(branch.refName.full) ?? [])
							: []),
					],
				),
			);

			return {
				stacks,
				navigationIndex: { items, indexByKey: buildIndexByKey(items, operandIdentityKey) },
			};
		},
	});

	return { ...content, isPending, isError };
};
