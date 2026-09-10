import {
	headInfoQueryOptions,
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { Stack, TargetCommit, TargetCommitPage, Worktree } from "@gitbutler/but-sdk";
import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { canLoadHistory, layout, layoutStructure, MORE_COMMITS, type Plan } from "./layout.ts";

const noWorktrees: ReadonlyArray<Worktree> = [];
const noCommits: ReadonlyArray<TargetCommit> = [];

/** The stacks graph as its host computes it once: the plan, the cards in its order, the linked worktrees, and the target line. */
export type Graph = ReturnType<typeof usePlan>;

/** The stacks graph's plan and the cards in its order. Called once per host, which hands it to the stacks. */
export const usePlan = (projectId: string) => {
	const dispatch = useAppDispatch();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: baseListing } = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const folds = useAppSelector((state) =>
		projectSlice.selectors.selectGraphFolds(state, projectId),
	);
	const listOrder = useMemo(() => headInfo?.stacks ?? [], [headInfo]);
	const target = headInfo?.target ?? null;
	const worktrees = headInfo?.worktrees ?? noWorktrees;
	const olderFrom = baseListing?.commits.at(-1)?.commit.id ?? "";
	const {
		data: olderData,
		fetchNextPage,
		hasNextPage,
		isFetching,
		isError,
	} = useInfiniteQuery({
		...olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom),
		enabled: (query) =>
			folds.historyExpanded &&
			target !== null &&
			olderFrom !== "" &&
			canLoadHistory(baseListing) &&
			(baseListing?.hasMore !== true ||
				query.state.data === undefined ||
				baseListing.commits.some((commit) => commit.inWorkspace) ||
				query.state.data.pages.some((page) => page.commits.some((commit) => commit.inWorkspace)) ||
				query.state.data.pages.at(-1)?.hasMore === true),
	});
	const olderPages = useMemo(
		() => olderData?.pages.flatMap((page) => page.commits) ?? [],
		[olderData],
	);
	// A clipped initial page still belongs to the upstream line until a shared
	// commit is reached; only continuation below its natural end is older History.
	const listing: TargetCommitPage | undefined = useMemo(
		() =>
			baseListing?.hasMore && olderData !== undefined
				? {
						commits: [...baseListing.commits, ...olderPages],
						hasMore: olderData.pages.at(-1)?.hasMore ?? false,
					}
				: baseListing,
		[baseListing, olderData, olderPages],
	);
	const historyPages = baseListing?.hasMore ? noCommits : olderPages;
	const structure = useMemo(
		() => layoutStructure(listOrder, listing, worktrees),
		[listOrder, listing, worktrees],
	);
	// Keep the plan stable: the rails re-measure whenever its identity changes.
	const plan: Plan = useMemo(
		() => layout(listOrder, target, listing, folds, worktrees, historyPages, structure),
		[listOrder, target, listing, folds, worktrees, historyPages, structure],
	);
	const stacks: Array<Stack> = useMemo(
		() =>
			structure.order.flatMap((index) => {
				const stack = listOrder[index];
				return stack === undefined ? [] : [stack];
			}),
		[structure.order, listOrder],
	);
	const showMoreHistory = async () => {
		if (isFetching || !plan.historyAvailable) return;
		if (
			isError ||
			(plan.historyHidden < MORE_COMMITS && (olderData === undefined || hasNextPage))
		) {
			const result = await fetchNextPage();
			if (result.isError) return;
		}
		if (plan.history.length > 0) dispatch(projectSlice.actions.showMoreGraphHistory({ projectId }));
	};
	return {
		plan,
		stacks,
		worktrees,
		listing,
		showMoreHistory,
		historyMore:
			!plan.historyAvailable || !plan.historyExpanded
				? ("hidden" as const)
				: isFetching
					? ("loading" as const)
					: isError
						? ("failed" as const)
						: plan.historyHidden > 0 || hasNextPage
							? ("idle" as const)
							: ("hidden" as const),
	};
};
