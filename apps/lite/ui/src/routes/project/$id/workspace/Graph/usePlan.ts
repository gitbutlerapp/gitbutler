import {
	headInfoQueryOptions,
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { Stack } from "@gitbutler/but-sdk";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { FIRST, layout, MORE, type Plan } from "./layout.ts";

/** The stacks graph as its host computes it once: the plan, the cards in its order, and the older history's paging. */
export type Graph = ReturnType<typeof usePlan>;

/**
 * The stacks graph's plan from the workspace's data and the fold state, with
 * the cards in the order the plan draws them. Called once per host, the page
 * and the harness panel, which build the applied address space from it and
 * hand it to the stacks to render: a second call would page the older
 * history twice.
 */
export const usePlan = (projectId: string) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: listing } = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const folds = useAppSelector((state) =>
		projectSlice.selectors.selectGraphFolds(state, projectId),
	);
	// Older history below the deepest fork point: the listing's own tail first,
	// then pages shared with the Upstream tab, the first of them fetched as
	// the base opens and the rest on demand.
	const olderFrom = listing?.commits.at(-1)?.commit.id ?? "";
	const olderOptions = olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom);
	const olderQuery = useInfiniteQuery({
		...olderOptions,
		enabled: folds.baseExpanded && olderFrom !== "",
	});
	// Folding the base forgets the pages it loaded, so it opens from scratch
	// next time rather than as long as it was left.
	const queryClient = useQueryClient();
	const forgetOlder = () => queryClient.removeQueries({ queryKey: olderOptions.queryKey });
	const olderPagesData = olderQuery.data;
	const olderPages = useMemo(
		() => olderPagesData?.pages.flatMap((page) => page.commits) ?? [],
		[olderPagesData],
	);
	const listOrder = useMemo(() => (headInfo?.stacks ?? []).toReversed(), [headInfo]);
	const target = headInfo?.target ?? null;
	// Explicit: the compiler does not memoise calls to imported functions, and
	// the rails re-measure whenever the plan's identity changes.
	const plan: Plan = useMemo(
		() => layout(listOrder, target, listing, folds, olderPages),
		[listOrder, target, listing, folds, olderPages],
	);
	// Pages come as the rows shown ask for them: the first as the base opens,
	// and another whenever an ask outruns what is loaded.
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
	return { plan, stacks, olderQuery, olderFrom, forgetOlder };
};
