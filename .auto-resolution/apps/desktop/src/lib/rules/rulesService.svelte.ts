import {
	isAiRule,
	type CreateRuleRequest,
	type UpdateRuleRequest,
	type WorkspaceRule,
	type WorkspaceRuleId
} from '$lib/rules/rule';
import { hasBackendExtra } from '$lib/state/backendQuery';
import { invalidatesList, providesItems, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
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

	get updateWorkspaceRuleMutate() {
		return this.api.endpoints.updateWorkspaceRule.mutate;
	}

	workspaceRules(projectId: string) {
		return this.api.endpoints.listWorkspaceRules.useQuery({ projectId });
	}

	hasRulesToClear(projectId: string, stackId?: string) {
		return this.api.endpoints.listWorkspaceRules.useQuery(
			{ projectId },
			{
				transform: (result) => {
					const allRules = workspaceRulesSelectors.selectAll(result);
					return allRules.some(
						(r) => isAiRule(r) && r.action.subject.subject.target.subject === stackId
					);
				}
			}
		);
	}

	aiSessionId(projectId: string, stackId?: string) {
		return this.api.endpoints.listWorkspaceRules.useQuery(
			{ projectId },
			{
				transform: (result) => {
					const allRules = workspaceRulesSelectors.selectAll(result);
					const rule = allRules.find(
						(r) => isAiRule(r) && r.action.subject.subject.target.subject === stackId
					);
					const sessionId = rule?.filters.at(0)?.subject;
					if (typeof sessionId === 'string') {
						return sessionId;
					}
				}
			}
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
				// Note: We don't invalidate WorkspaceRules here - the backend listener handles it
				// This prevents double-invalidation which causes cache to blink
				invalidatesTags: () => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks) // Probably this is still needed??
				]
			}),
			deleteWorkspaceRule: build.mutation<void, { projectId: string; id: WorkspaceRuleId }>({
				extraOptions: { command: 'delete_workspace_rule' },
				query: (args) => args,
				// Note: We don't invalidate WorkspaceRules here - the backend listener handles it
				// This prevents double-invalidation which causes cache to blink
				invalidatesTags: [
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
				// Note: We don't invalidate WorkspaceRules here - the backend listener handles it
				// This prevents double-invalidation which causes cache to blink
				invalidatesTags: () => [
					invalidatesList(ReduxTag.WorktreeChanges),
					invalidatesList(ReduxTag.Stacks) // Probably this is still needed??
				]
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

export const workspaceRulesSelectors = workspaceRulesAdapter.getSelectors();
