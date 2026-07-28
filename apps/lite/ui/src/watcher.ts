import type { QueryKey } from "#ui/api/queries.ts";
import type { WatcherEvent } from "@gitbutler/but-sdk";
import type { QueryClient } from "@tanstack/react-query";

export const handleWatcher = (
	event: WatcherEvent,
	projectId: string,
	client: QueryClient,
): void => {
	switch (event.payload.type) {
		case "gitFetch":
			void client.invalidateQueries({
				queryKey: ["workspaceFetchStatus" satisfies QueryKey, projectId],
			});
			void client.invalidateQueries({ queryKey: ["reviews" satisfies QueryKey, projectId] });
			// A fetch moves remote-tracking refs, so the branch listing goes stale,
			// and it can advance a remote branch whose commits or diff are already
			// cached (unfolded or selected in the Branches tab). This case does not
			// fall through to the gitActivity invalidations below.
			void client.invalidateQueries({ queryKey: ["branchList" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["branchDetails" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["branchDiff" satisfies QueryKey, projectId] });
			break;
		case "gitActivity":
		case "workspaceActivity": {
			void client.invalidateQueries({ queryKey: ["absorptionPlan" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["branchDetails" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["branchList" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["branchDiff" satisfies QueryKey, projectId] });
			void client.invalidateQueries({
				queryKey: ["changesInWorktree" satisfies QueryKey, projectId],
			});
			void client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, projectId] });
			void client.invalidateQueries({
				queryKey: ["commitDetailsWithLineStats" satisfies QueryKey, projectId],
			});
			void client.invalidateQueries({ queryKey: ["dryRun" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["headInfo" satisfies QueryKey, projectId] });
			void client.invalidateQueries({
				queryKey: ["treeChangeDiffs" satisfies QueryKey, projectId],
			});
			break;
		}
		case "worktreeChanges":
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(
				["changesInWorktree" satisfies QueryKey, projectId],
				() => workspaceChanges,
			);
			void client.invalidateQueries({ queryKey: ["absorptionPlan" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["comments" satisfies QueryKey, projectId] });
			void client.invalidateQueries({ queryKey: ["dryRun" satisfies QueryKey, projectId] });
			void client.invalidateQueries({
				queryKey: ["treeChangeDiffs" satisfies QueryKey, projectId],
			});
			break;
	}
};
