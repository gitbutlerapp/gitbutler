import {
	headInfoQueryOptions,
	guiSettingsQueryOptions,
	olderTargetCommitsInfiniteQueryOptions,
	workspaceTargetCommitsQueryOptions,
} from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { defaultSettings } from "#ui/settings.ts";
import { useNow } from "#ui/components/useNow.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import type { Stack, TargetCommit, TargetCommitPage, Worktree } from "@gitbutler/but-sdk";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { canLoadHistory, layout, layoutStructure, MORE_COMMITS, type Plan } from "./layout.ts";

const noWorktrees: ReadonlyArray<Worktree> = [];
const noCommits: ReadonlyArray<TargetCommit> = [];

/** The stacks graph as its host computes it once: the plan, the cards in its order, the linked worktrees, and the target line. */
export type Graph = ReturnType<typeof usePlan>;

/** The stacks graph's plan and the cards in its order. Called once per host, which hands it to the stacks. */
export const usePlan = (projectId: string) => {
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: baseListing } = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const folds = useAppSelector((state) =>
		projectSlice.selectors.selectGraphFolds(state, projectId),
	);
	const { data: historyDisplayMode = defaultSettings.historyDisplayMode } = useQuery({
		...guiSettingsQueryOptions,
		select: (settings) => settings.historyDisplayMode ?? defaultSettings.historyDisplayMode,
	});
	const recent = historyDisplayMode === "last-12-hours";
	const now = useNow(recent && folds.historyExpanded ? 60_000 : null);
	const historySince = recent ? now - 12 * 60 * 60 * 1000 : undefined;
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
		...olderTargetCommitsInfiniteQueryOptions(projectId, olderFrom, recent),
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
		() =>
			layout(
				listOrder,
				target,
				listing,
				{ ...folds, historySince },
				worktrees,
				historyPages,
				structure,
			),
		[listOrder, target, listing, folds, historySince, worktrees, historyPages, structure],
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
			const currentMode =
				queryClient.getQueryData(guiSettingsQueryOptions.queryKey)?.historyDisplayMode ??
				defaultSettings.historyDisplayMode;
			if (currentMode !== historyDisplayMode) return;
		}
		if (plan.history.length > 0 || plan.historyHidden > 0)
			dispatch(projectSlice.actions.showMoreGraphHistory({ projectId }));
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
