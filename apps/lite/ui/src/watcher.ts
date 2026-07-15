import { liteApi, type QueryTag } from "#ui/api/queries.ts";
import type { AppDispatch } from "#ui/store.ts";
import { WatcherEvent } from "@gitbutler/but-sdk";

const projectTags = (projectId: string, types: Array<QueryTag>) =>
	types.map((type) => ({ type, id: projectId }));

export const handleWatcher = (
	event: WatcherEvent,
	projectId: string,
	dispatch: AppDispatch,
): void => {
	switch (event.payload.type) {
		case "gitFetch":
			dispatch(liteApi.util.invalidateTags([{ type: "Reviews", id: projectId }]));
			break;
		case "gitActivity":
		case "workspaceActivity":
			void dispatch(
				liteApi.util.invalidateTags(
					projectTags(projectId, [
						"AbsorptionPlan",
						"Branches",
						"BranchDetails",
						"BranchDiff",
						"ChangesInWorktree",
						"CommitDetailsWithLineStats",
						"DryRun",
						"HeadInfo",
						"TreeChangeDiffs",
					]),
				),
			);
			break;
		case "worktreeChanges":
			void dispatch(
				liteApi.util.upsertQueryData("changesInWorktree", projectId, event.payload.subject.changes),
			);
			dispatch(
				liteApi.util.invalidateTags(
					projectTags(projectId, ["AbsorptionPlan", "DryRun", "TreeChangeDiffs"]),
				),
			);
			break;
	}
};
