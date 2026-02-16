import { invalidatesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { TreeChange } from "$lib/hunks/change";
import type { BackendApi, ClientState } from "$lib/state/clientState.svelte";
import type { HunkAssignment, Action } from "@gitbutler/core/api";

type ChatMessage = {
	type: "user" | "assistant";
	content: string;
};

export const ACTION_SERVICE = new InjectionToken<ActionService>("ActionService");

export class ActionService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		backendApi: BackendApi,
		private backend: IBackend,
	) {
		this.api = injectEndpoints(backendApi);
	}

	get autoCommit() {
		return this.api.endpoints.autoCommit.useMutation();
	}

	listenForAutoCommit(projectId: string, listen: (event: Action.AutoCommitEvent) => void) {
		const unlisten = this.backend.listen(`project://${projectId}/auto-commit`, (event) => {
			const payload = event.payload as Action.AutoCommitEvent;
			listen(payload);
		});

		return unlisten;
	}

	get branchChanges() {
		return this.api.endpoints.autoBranchChanges.useMutation();
	}

	get bot() {
		return this.api.endpoints.bot.useMutation();
	}
}

function injectEndpoints(api: ClientState["backendApi"]) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			autoCommit: build.mutation<
				void,
				{ projectId: string; target: HunkAssignment.AbsorptionTarget; useAi: boolean }
			>({
				extraOptions: {
					command: "auto_commit",
					actionName: "Figure out where to commit the given changes",
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
				],
			}),
			autoBranchChanges: build.mutation<
				void,
				{ projectId: string; changes: TreeChange[]; model: string }
			>({
				extraOptions: {
					command: "auto_branch_changes",
					actionName: "Create a branch for the given changes",
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
				],
			}),
			bot: build.mutation<
				string,
				{ projectId: string; messageId: string; chatMessages: ChatMessage[]; model: string }
			>({
				extraOptions: {
					command: "bot",
					actionName: "but bot action",
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.HeadSha),
					invalidatesList(ReduxTag.WorktreeChanges),
				],
			}),
		}),
	});
}
