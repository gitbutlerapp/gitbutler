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
import { branchAddress, commitAddress, addressIdentityKey, type Address } from "#ui/addresses.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import { buildIndexByKey, type AddressSpace } from "#ui/workspace/address-space.ts";
import type { Commit, ListedBranch } from "@gitbutler/but-sdk";
import { useQueries, useQuery, type UseQueryResult } from "@tanstack/react-query";
import { useDeferredValue } from "react";

type BranchesListBranch = {
	branch: ListedBranch;
	addressIndex: number;
	/**
	 * `undefined` means that the branch is folded, or that it's unfolded and that the query is either
	 * loading or failed.
	 */
	commits: Array<Commit> | undefined;
};

type BranchesListStack = {
	branches: Array<BranchesListBranch>;
	commitCount: number;
};

export type BranchesListContent = {
	stacks: Array<BranchesListStack>;
	stackIndexByAddressIndex: Array<number>;
	addressSpace: AddressSpace<Address>;
};

export const emptyBranchesListContent: BranchesListContent = {
	stacks: [],
	stackIndexByAddressIndex: [],
	addressSpace: { items: [], indexByKey: new Map() },
};

/**
 * The branches page's visible unapplied stacks and the matching address space.
 *
 * This is the single source of truth for what the page shows: both the list
 * rendering and the selection resolution in the workspace page consume it, so
 * filtering and fold state cannot drift between the two.
 */
export const useBranchesList = (projectId: string): UseQueryResult<BranchesListContent> => {
	const active = usePage() === "branches";
	const filters = useAppSelector((state) =>
		projectSlice.selectors.selectBranchFilters(state, projectId),
	);
	// Deferred so the fuzzy filter runs at low priority and typing stays
	// responsive; the input itself is controlled by the non-deferred value. A
	// closed filter narrows nothing, so it reads the same here as an empty query.
	const search = useDeferredValue(
		useAppSelector((state) => projectSlice.selectors.selectBranchSearch(state, projectId)) ?? "",
	);
	const unfoldedBranches = useAppSelector((state) =>
		projectSlice.selectors.selectUnfoldedBranches(state, projectId),
	);

	const unfoldedBranchRefs = Object.keys(unfoldedBranches);
	const commitsByRef = useQueries({
		queries: unfoldedBranchRefs.map((refName) => ({
			...branchDetailsQueryOptions({ projectId, ...branchDetailsParams(refName) }),
			enabled: active,
		})),
		combine: (results) =>
			new Map(unfoldedBranchRefs.map((refName, index) => [refName, results[index]?.data?.commits])),
	});

	// The whole derivation lives in `select` so its result keeps a stable
	// identity: react-query caches it on the query data and the `select`
	// reference, and React Compiler memoizes this inline closure by its captured
	// inputs — so the closure, and thus the cached result, only changes when an
	// input like `search` or `showEmpty` does. Deriving in render instead would
	// rebuild the address space every pass and re-render every row that reads
	// it through context.
	return useQuery({
		...branchListQueryOptions(projectId),
		enabled: active,
		select: (listedStacks): BranchesListContent => {
			const unapplied = searchStacks(unappliedStacks(listedStacks, filters), search);
			const items: Array<Address> = [];
			const stackIndexByAddressIndex: Array<number> = [];
			const stacks = unapplied.map((stack, stackIndex): BranchesListStack => {
				let commitCount = 0;
				const branches = stack.branches.map((branch): BranchesListBranch => {
					const addressIndex = items.length;
					items.push(branchAddress({ branchRef: encodeBytes(branch.refName.full) }));
					stackIndexByAddressIndex.push(stackIndex);

					const branchCommits = commitsByRef.get(branch.refName.full);
					const commits =
						unfoldedBranches[branch.refName.full] &&
						!branchIsEmpty(branch) &&
						branchCommits !== undefined
							? branchOwnCommits(branch, branchCommits)
							: undefined;

					if (commits !== undefined) {
						commitCount += commits.length;
						for (const commit of commits) {
							items.push(commitAddress({ commitId: commit.id, changeId: commit.changeId }));
							stackIndexByAddressIndex.push(stackIndex);
						}
					}

					return { branch, addressIndex, commits };
				});

				return { branches, commitCount };
			});

			return {
				stacks,
				stackIndexByAddressIndex,
				addressSpace: { items, indexByKey: buildIndexByKey(items, addressIdentityKey) },
			};
		},
	});
};
