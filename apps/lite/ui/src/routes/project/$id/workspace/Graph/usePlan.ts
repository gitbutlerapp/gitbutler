import { headInfoQueryOptions, workspaceTargetCommitsQueryOptions } from "#ui/api/queries.ts";
import { projectSlice } from "#ui/projects/state.ts";
import { useAppSelector } from "#ui/store.ts";
import type { Stack, Worktree } from "@gitbutler/but-sdk";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { layout, type Plan } from "./layout.ts";

const noWorktrees: ReadonlyArray<Worktree> = [];

/** The stacks graph as its host computes it once: the plan, the cards in its order, the linked worktrees, and the target line. */
export type Graph = ReturnType<typeof usePlan>;

/** The stacks graph's plan and the cards in its order. Called once per host, which hands it to the stacks. */
export const usePlan = (projectId: string) => {
	const { data: headInfo } = useQuery(headInfoQueryOptions(projectId));
	const { data: listing } = useQuery(workspaceTargetCommitsQueryOptions(projectId));
	const folds = useAppSelector((state) =>
		projectSlice.selectors.selectGraphFolds(state, projectId),
	);
	const listOrder = useMemo(() => headInfo?.stacks ?? [], [headInfo]);
	const target = headInfo?.target ?? null;
	const worktrees = headInfo?.worktrees ?? noWorktrees;
	// Explicit: the compiler does not memoise imported calls, and the rails re-measure on every new plan.
	const plan: Plan = useMemo(
		() => layout(listOrder, target, listing, folds, worktrees),
		[listOrder, target, listing, folds, worktrees],
	);
	const stacks: Array<Stack> = useMemo(
		() =>
			plan.order.flatMap((index) => {
				const stack = listOrder[index];
				return stack === undefined ? [] : [stack];
			}),
		[plan, listOrder],
	);
	return { plan, stacks, worktrees, listing };
};
