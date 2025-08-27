import { invalidatesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import type { TreeChange } from '$lib/hunks/change';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

type ChatMessage = {
	type: 'user' | 'assistant';
	content: string;
};

export const ACTION_SERVICE = new InjectionToken<ActionService>('ActionService');

export class ActionService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get autoCommit() {
		return this.api.endpoints.autoCommit.useMutation();
	}

	get branchChanges() {
		return this.api.endpoints.autoBranchChanges.useMutation();
	}

	get absorb() {
		return this.api.endpoints.absorb.useMutation();
	}

	get freestyle() {
		return this.api.endpoints.freestyle.useMutation();
	}

	get bot() {
		return this.api.endpoints.bot.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			autoCommit: build.mutation<void, { projectId: string; changes: TreeChange[] }>({
				extraOptions: {
					command: 'auto_commit',
					actionName: 'Figure out where to commit the given changes'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			}),
			autoBranchChanges: build.mutation<void, { projectId: string; changes: TreeChange[] }>({
				extraOptions: {
					command: 'auto_branch_changes',
					actionName: 'Create a branch for the given changes'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			}),
			absorb: build.mutation<void, { projectId: string; changes: TreeChange[] }>({
				extraOptions: {
					command: 'absorb',
					actionName: 'Absorb changes into the best matching branch and commit'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			}),
			freestyle: build.mutation<
				string,
				{ projectId: string; messageId: string; chatMessages: ChatMessage[]; model: string | null }
			>({
				extraOptions: {
					command: 'freestyle',
					actionName: 'Perform a freestyle action based on the given prompt'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			}),
			bot: build.mutation<
				string,
				{ projectId: string; messageId: string; chatMessages: ChatMessage[] }
			>({
				extraOptions: {
					command: 'bot',
					actionName: 'but bot action'
				},
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			})
		})
	});
}
