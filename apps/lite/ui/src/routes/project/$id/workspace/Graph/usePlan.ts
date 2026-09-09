import {
	headInfoQueryOptions,
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { Stack, Worktree } from "@gitbutler/but-sdk";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { FIRST, layout, MORE, type Plan } from "./layout.ts";

const noWorktrees: ReadonlyArray<Worktree> = [];

/** The stacks graph as its host computes it once: the plan, the cards in its order, the linked worktrees, and the older history's paging. */
export type Graph = ReturnType<typeof usePlan>;

/**
 * The stacks graph's plan and the cards in its order. Called once per host,
 * which hands it to the stacks: a second call would page the older history
 * twice.
 */
export const usePlan = (projectId: string) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: listing } = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const folds = useAppSelector((state) =>
		projectSlice.selectors.selectGraphFolds(state, projectId),
	);
	// Older history: pages shared with the Upstream tab, fetched as the base opens and on demand.
	const olderFrom = listing?.commits.at(-1)?.commit.id ?? "";
	const olderOptions = olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom);
	const olderQuery = useInfiniteQuery({
		...olderOptions,
		enabled: folds.baseExpanded && olderFrom !== "",
	});
	// Folding the base forgets its pages, so it reopens from scratch.
	const queryClient = useQueryClient();
	const forgetOlder = () => queryClient.removeQueries({ queryKey: olderOptions.queryKey });
	const olderPagesData = olderQuery.data;
	const olderPages = useMemo(
		() => olderPagesData?.pages.flatMap((page) => page.commits) ?? [],
		[olderPagesData],
	);
	const listOrder = useMemo(() => headInfo?.stacks ?? [], [headInfo]);
	const target = headInfo?.target ?? null;
	const worktrees = headInfo?.worktrees ?? noWorktrees;
	// Explicit: the compiler does not memoise imported calls, and the rails re-measure on every new plan.
	const plan: Plan = useMemo(
		() => layout(listOrder, target, listing, folds, olderPages, worktrees),
		[listOrder, target, listing, folds, olderPages, worktrees],
	);
	// Fetch a page whenever an ask outruns what is loaded.
	const { fetchNextPage, hasNextPage, isFetching } = olderQuery;
	const loaded = plan.older.length + plan.olderHidden;
	const wanted = folds.baseExpanded ? FIRST + folds.moreOlder * MORE : 0;
	useEffect(() => {
		if (loaded < wanted && hasNextPage && !isFetching) void fetchNextPage();
	}, [loaded, wanted, hasNextPage, isFetching, fetchNextPage]);
	const stacks: Array<Stack> = useMemo(
		() =>
			plan.order.flatMap((index) => {
				const stack = listOrder[index];
				return stack === undefined ? [] : [stack];
			}),
		[plan, listOrder],
	);
	return { plan, stacks, worktrees, olderQuery, olderFrom, forgetOlder };
};
