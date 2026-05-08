import { hasBackendExtra } from "$lib/state/backendQuery";
import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { ConflictEntryPresence, HeadAndMode, HeadSha, TreeChange } from "@gitbutler/but-sdk";

export function buildModeEndpoints(build: BackendEndpointBuilder) {
	return {
		enterEditMode: build.mutation<void, { projectId: string; commitId: string; stackId: string }>({
			extraOptions: { command: "enter_edit_mode" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.InitalEditListing),
				invalidatesList(ReduxTag.EditChangesSinceInitial),
				invalidatesList(ReduxTag.HeadMetadata),
			],
		}),
		abortEditAndReturnToWorkspace: build.mutation<void, { projectId: string; force: boolean }>({
			extraOptions: { command: "abort_edit_and_return_to_workspace" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.HeadMetadata)],
		}),
		saveEditAndReturnToWorkspace: build.mutation<void, { projectId: string }>({
			extraOptions: { command: "save_edit_and_return_to_workspace" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.HeadSha),
				invalidatesList(ReduxTag.HeadMetadata),
			],
		}),
		initialEditModeState: build.query<
			[TreeChange, ConflictEntryPresence | undefined][],
			{ projectId: string }
		>({
			extraOptions: { command: "edit_initial_index_state" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.InitalEditListing)],
		}),
		changesSinceInitialEditState: build.query<TreeChange[], { projectId: string }>({
			extraOptions: { command: "edit_changes_from_initial" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.EditChangesSinceInitial)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { invoke, listen } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;
				let finished = false;
				const unsubscribe = listen<unknown>(
					`project://${arg.projectId}/worktree_changes`,
					async (_) => {
						if (finished) return;
						const changes = await invoke<TreeChange[]>("edit_changes_from_initial", arg);
						lifecycleApi.updateCachedData(() => changes);
					},
				);
				await lifecycleApi.cacheEntryRemoved;
				finished = true;
				unsubscribe();
			},
		}),
		headAndMode: build.query<HeadAndMode, { projectId: string }>({
			extraOptions: { command: "operating_mode" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.HeadMetadata)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				const { invoke, listen } = lifecycleApi.extra.backend;
				await lifecycleApi.cacheDataLoaded;
				let finished = false;

				// Re-invoke the command on events to get fresh data including divergence.
				async function refresh() {
					if (finished) return;
					const result = await invoke<HeadAndMode>("operating_mode", arg);
					lifecycleApi.updateCachedData(() => result);
				}

				const unsub1 = listen<unknown>(`project://${arg.projectId}/git/head`, refresh);
				const unsub2 = listen<unknown>(
					`project://${arg.projectId}/git/workspace-ref-changed`,
					refresh,
				);

				await lifecycleApi.cacheEntryRemoved;
				finished = true;
				unsub1();
				unsub2();
			},
		}),
		headSha: build.query<HeadSha, { projectId: string }>({
			extraOptions: { command: "head_sha" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.HeadSha)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				await lifecycleApi.cacheDataLoaded;
				const unsubscribe = lifecycleApi.extra.backend.listen<HeadSha>(
					`project://${arg.projectId}/git/activity`,
					(event) => {
						lifecycleApi.updateCachedData(() => event.payload);
					},
				);
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
		}),
	};
}
