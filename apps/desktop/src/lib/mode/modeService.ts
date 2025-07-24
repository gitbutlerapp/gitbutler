import { hasTauriExtra } from '$lib/state/backendQuery';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import type { ConflictEntryPresence } from '$lib/conflictEntryPresence';
import type { TreeChange } from '$lib/hunks/change';
import type { ClientState } from '$lib/state/clientState.svelte';

export interface EditModeMetadata {
	commitOid: string;
	branchReference: string;
}

export interface OutsideWorkspaceMetadata {
	/** The name of the currently checked out branch or null if in detached head state. */
	branchName: string | null;
	/** The paths of any files that would conflict with the workspace as it currently is */
	worktreeConflicts: string[];
}

type Mode =
	| { type: 'OpenWorkspace' }
	| {
			type: 'OutsideWorkspace';
			subject: OutsideWorkspaceMetadata;
	  }
	| {
			type: 'Edit';
			subject: EditModeMetadata;
	  };
interface HeadAndMode {
	head?: string;
	operatingMode?: Mode;
}

export const MODE_SERVICE = new InjectionToken<ModeService>('ModeService');

export class ModeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState['backendApi']) {
		this.api = injectEndpoints(state);
	}

	get enterEditMode() {
		return this.api.endpoints.enterEditMode.mutate;
	}

	get abortEditAndReturnToWorkspace() {
		return this.api.endpoints.abortEditAndReturnToWorkspace.mutate;
	}

	get saveEditAndReturnToWorkspace() {
		return this.api.endpoints.saveEditAndReturnToWorkspace.mutate;
	}

	get initialEditModeState() {
		return this.api.endpoints.initialEditModeState.useQueryStore;
	}

	get changesSinceInitialEditState() {
		return this.api.endpoints.changesSinceInitialEditState.useQueryStore;
	}

	get mode() {
		return this.api.endpoints.mode.useQuery;
	}

	get head() {
		return this.api.endpoints.mode.useQuery;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			enterEditMode: build.mutation<void, { projectId: string; commitId: string; stackId: string }>(
				{
					extraOptions: { command: 'enter_edit_mode' },
					query: (args) => args,
					invalidatesTags: [
						invalidatesList(ReduxTag.InitalEditListing),
						invalidatesList(ReduxTag.EditChangesSinceInitial),
						invalidatesList(ReduxTag.HeadMetadata)
					]
				}
			),
			abortEditAndReturnToWorkspace: build.mutation<void, { projectId: string }>({
				extraOptions: { command: 'abort_edit_and_return_to_workspace' },
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.InitalEditListing),
					invalidatesList(ReduxTag.EditChangesSinceInitial),
					invalidatesList(ReduxTag.HeadMetadata)
				]
			}),
			saveEditAndReturnToWorkspace: build.mutation<void, { projectId: string }>({
				extraOptions: { command: 'save_edit_and_return_to_workspace' },
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.InitalEditListing),
					invalidatesList(ReduxTag.EditChangesSinceInitial),
					invalidatesList(ReduxTag.HeadMetadata)
				]
			}),
			initialEditModeState: build.query<
				[TreeChange, ConflictEntryPresence | undefined][],
				{ projectId: string }
			>({
				extraOptions: { command: 'edit_initial_index_state' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.InitalEditListing)]
			}),
			changesSinceInitialEditState: build.query<TreeChange[], { projectId: string }>({
				extraOptions: { command: 'edit_changes_from_initial' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.EditChangesSinceInitial)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					const { invoke, listen } = lifecycleApi.extra.tauri;
					await lifecycleApi.cacheDataLoaded;
					// We are listening to this only for the notification that changes have been made
					const unsubscribe = listen<unknown>(
						`project://${arg.projectId}/worktree_changes`,
						async (_) => {
							const changes = await invoke<TreeChange[]>('edit_changes_from_initial', arg);
							lifecycleApi.updateCachedData(() => changes);
						}
					);
					// The `cacheEntryRemoved` promise resolves when the result is removed
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			}),
			mode: build.query<Mode, { projectId: string }>({
				extraOptions: { command: 'operating_mode' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.HeadMetadata)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<HeadAndMode>(
						`project://${arg.projectId}/git/head`,
						(event) => {
							lifecycleApi.updateCachedData(() => event.payload.operatingMode);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			}),
			head: build.query<string, { projectId: string }>({
				extraOptions: { command: 'git_head' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.HeadMetadata)],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<HeadAndMode>(
						`project://${arg.projectId}/git/head`,
						(event) => {
							lifecycleApi.updateCachedData(() => event.payload.head);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			})
		})
	});
}
