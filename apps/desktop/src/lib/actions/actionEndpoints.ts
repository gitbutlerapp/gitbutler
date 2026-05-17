import {
	type CreateRuleRequest,
	type UpdateRuleRequest,
	type WorkspaceRule,
	type WorkspaceRuleId,
} from "$lib/rules/rule";
import { invalidatesList, providesItems, ReduxTag } from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";

export function buildActionEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Rules ───────────────────────────────────────────────────
		createWorkspaceRule: build.mutation<
			WorkspaceRule,
			{ projectId: string; request: CreateRuleRequest }
		>({
			extraOptions: { command: "create_workspace_rule" },
			query: (args) => args,
			invalidatesTags: () => [
				invalidatesList(ReduxTag.WorkspaceRules),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
			],
		}),
		deleteWorkspaceRule: build.mutation<void, { projectId: string; ruleId: WorkspaceRuleId }>({
			extraOptions: { command: "delete_workspace_rule" },
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorkspaceRules)],
		}),
		updateWorkspaceRule: build.mutation<
			WorkspaceRule,
			{ projectId: string; request: UpdateRuleRequest }
		>({
			extraOptions: { command: "update_workspace_rule" },
			query: (args) => args,
			invalidatesTags: () => [
				invalidatesList(ReduxTag.WorkspaceRules),
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
			],
		}),
		listWorkspaceRules: build.query<
			EntityState<WorkspaceRule, WorkspaceRuleId>,
			{ projectId: string }
		>({
			extraOptions: { command: "list_workspace_rules" },
			query: (args) => args,
			providesTags: (result) => providesItems(ReduxTag.WorkspaceRules, result?.ids ?? []),
			transformResponse: (response: WorkspaceRule[]) => {
				return workspaceRulesAdapter.addMany(workspaceRulesAdapter.getInitialState(), response);
			},
		}),
	};
}

const workspaceRulesAdapter = createEntityAdapter<WorkspaceRule, WorkspaceRuleId>({
	selectId: (rule) => rule.id,
});

export const workspaceRulesSelectors = workspaceRulesAdapter.getSelectors();
