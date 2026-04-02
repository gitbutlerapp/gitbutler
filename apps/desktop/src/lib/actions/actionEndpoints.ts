import {
	type CreateRuleRequest,
	type UpdateRuleRequest,
	type WorkspaceRule,
	type WorkspaceRuleId,
} from "$lib/rules/rule";
import { invalidatesList, providesItems, ReduxTag } from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type { AbsorptionTarget, TreeChange } from "@gitbutler/but-sdk";

type ChatMessage = {
	type: "user" | "assistant";
	content: string;
};

export function buildActionEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Actions ─────────────────────────────────────────────────
		autoCommit: build.mutation<
			void,
			{ projectId: string; target: AbsorptionTarget; useAi: boolean }
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

		// ── Rules ───────────────────────────────────────────────────
		createWorkspaceRule: build.mutation<
			WorkspaceRule,
			{ projectId: string; request: CreateRuleRequest }
		>({
			extraOptions: { command: "create_workspace_rule" },
			query: (args) => args,
			invalidatesTags: () => [
				invalidatesList(ReduxTag.WorktreeChanges),
				invalidatesList(ReduxTag.Stacks),
			],
		}),
		deleteWorkspaceRule: build.mutation<void, { projectId: string; ruleId: WorkspaceRuleId }>({
			extraOptions: { command: "delete_workspace_rule" },
			query: (args) => args,
			invalidatesTags: [
				invalidatesList(ReduxTag.ClaudeCodeTranscript),
				invalidatesList(ReduxTag.ClaudePermissionRequests),
				invalidatesList(ReduxTag.ClaudeSessionDetails),
				invalidatesList(ReduxTag.ClaudeStackActive),
			],
		}),
		updateWorkspaceRule: build.mutation<
			WorkspaceRule,
			{ projectId: string; request: UpdateRuleRequest }
		>({
			extraOptions: { command: "update_workspace_rule" },
			query: (args) => args,
			invalidatesTags: () => [
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
