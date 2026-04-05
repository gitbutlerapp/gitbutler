import { hasBackendExtra } from "$lib/state/backendQuery";
import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import type { ConflictEntryPresence } from "$lib/files/conflictEntryPresence";
import type { TreeChange } from "$lib/hunks/change";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

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
				await lifecycleApi.cacheDataLoaded;
				const unsubscribe = lifecycleApi.extra.backend.listen<HeadAndMode>(
					`project://${arg.projectId}/git/head`,
					(event) => {
						lifecycleApi.updateCachedData(() => event.payload);
					},
				);
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
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

export interface EditModeMetadata {
	commitOid: string;
	branchReference: string;
}

export interface OutsideWorkspaceMetadata {
	branchName: string | null;
	worktreeConflicts: string[];
}

export type Mode =
	| { type: "OpenWorkspace" }
	| {
			type: "OutsideWorkspace";
			subject: OutsideWorkspaceMetadata;
	  }
	| {
			type: "Edit";
			subject: EditModeMetadata;
	  };

interface HeadAndMode {
	head?: string;
	operatingMode?: Mode;
}

interface HeadSha {
	headSha?: string;
}
