import { hasBackendExtra } from '$lib/state/backendQuery';
import { invalidatesItem, invalidatesList, providesItems, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type {
	CreateRuleRequest,
	UpdateRuleRequest,
	WorkspaceRule,
	WorkspaceRuleId
} from '$lib/rules/rule';
import type { BackendApi } from '$lib/state/clientState.svelte';

export const RULES_SERVICE = new InjectionToken<RulesService>('RulesService');

export default class RulesService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get createWorkspaceRule() {
		return this.api.endpoints.createWorkspaceRule.useMutation();
	}

	get deleteWorkspaceRule() {
		return this.api.endpoints.deleteWorkspaceRule.useMutation();
	}

	get deleteWorkspaceRuleMutate() {
		return this.api.endpoints.deleteWorkspaceRule.mutate;
	}

	get updateWorkspaceRule() {
		return this.api.endpoints.updateWorkspaceRule.useMutation();
	}

	workspaceRules(projectId: string) {
		return this.api.endpoints.listWorkspaceRules.useQuery(
			{ projectId },
			{ transform: (result) => workspaceRulesSelectors.selectAll(result) }
		);
	}

	async fetchListWorkspaceRules(projectId: string) {
		return await this.api.endpoints.listWorkspaceRules.fetch(
			{ projectId },
			{ transform: (result) => workspaceRulesSelectors.selectAll(result) }
		);
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			createWorkspaceRule: build.mutation<
				WorkspaceRule,
				{ projectId: string; request: CreateRuleRequest }
			>({
				extraOptions: { command: 'create_workspace_rule' },
				query: (args) => args,
				invalidatesTags: () => [
					invalidatesList(ReduxTag.WorkspaceRules),
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks)
				]
			}),
			deleteWorkspaceRule: build.mutation<void, { projectId: string; id: WorkspaceRuleId }>({
				extraOptions: { command: 'delete_workspace_rule' },
				query: (args) => args,
				invalidatesTags: [
					invalidatesList(ReduxTag.WorkspaceRules),
					invalidatesList(ReduxTag.ClaudeCodeTranscript),
					invalidatesList(ReduxTag.ClaudePermissionRequests),
					invalidatesList(ReduxTag.ClaudeSessionDetails),
					invalidatesList(ReduxTag.ClaudeStackActive)
				]
			}),
			updateWorkspaceRule: build.mutation<
				WorkspaceRule,
				{ projectId: string; request: UpdateRuleRequest }
			>({
				extraOptions: { command: 'update_workspace_rule' },
				query: (args) => args,
				invalidatesTags: (result) =>
					result
						? [
								invalidatesItem(ReduxTag.WorkspaceRules, result.id),
								invalidatesList(ReduxTag.WorkspaceRules),
								invalidatesList(ReduxTag.WorktreeChanges),
								invalidatesList(ReduxTag.Stacks)
							]
						: []
			}),
			listWorkspaceRules: build.query<
				EntityState<WorkspaceRule, WorkspaceRuleId>,
				{ projectId: string }
			>({
				extraOptions: { command: 'list_workspace_rules' },
				query: (args) => args,
				providesTags: (result) => providesItems(ReduxTag.WorkspaceRules, result?.ids ?? []),
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasBackendExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Backend not found!');
					}
					// The `cacheDataLoaded` promise resolves when the result is first loaded.
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.backend.listen(
						`project://${arg.projectId}/rule-updates`,
						() => {
							lifecycleApi.dispatch(
								api.util.invalidateTags([invalidatesList(ReduxTag.WorkspaceRules)])
							);
						}
					);
					// The `cacheEntryRemoved` promise resolves when the result is removed
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
				transformResponse: (response: WorkspaceRule[]) => {
					return workspaceRulesAdapter.addMany(workspaceRulesAdapter.getInitialState(), response);
				}
			})
		})
	});
}

const workspaceRulesAdapter = createEntityAdapter<WorkspaceRule, WorkspaceRuleId>({
	selectId: (rule) => rule.id
});

const workspaceRulesSelectors = workspaceRulesAdapter.getSelectors();
