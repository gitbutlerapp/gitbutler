import { workspaceRulesSelectors } from "$lib/actions/actionEndpoints";
import { isAiRule } from "$lib/rules/rule";
export { workspaceRulesSelectors } from "$lib/actions/actionEndpoints";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";

export const RULES_SERVICE = new InjectionToken<RulesService>("RulesService");

export default class RulesService {
	constructor(private backendApi: BackendApi) {}

	get createWorkspaceRule() {
		return this.backendApi.endpoints.createWorkspaceRule.useMutation();
	}

	get deleteWorkspaceRule() {
		return this.backendApi.endpoints.deleteWorkspaceRule.useMutation();
	}

	get deleteWorkspaceRuleMutate() {
		return this.backendApi.endpoints.deleteWorkspaceRule.mutate;
	}

	get updateWorkspaceRule() {
		return this.backendApi.endpoints.updateWorkspaceRule.useMutation();
	}

	get updateWorkspaceRuleMutate() {
		return this.backendApi.endpoints.updateWorkspaceRule.mutate;
	}

	workspaceRules(projectId: string) {
		return this.backendApi.endpoints.listWorkspaceRules.useQuery({ projectId });
	}

	hasRulesToClear(projectId: string, stackId?: string) {
		return this.backendApi.endpoints.listWorkspaceRules.useQuery(
			{ projectId },
			{
				transform: (result) => {
					const allRules = workspaceRulesSelectors.selectAll(result);
					return allRules.some(
						(r) => isAiRule(r) && r.action.subject.subject.target.subject === stackId,
					);
				},
			},
		);
	}

	aiSessionId(projectId: string, stackId?: string) {
		return this.backendApi.endpoints.listWorkspaceRules.useQuery(
			{ projectId },
			{
				transform: (result) => {
					const allRules = workspaceRulesSelectors.selectAll(result);
					const rule = allRules.find(
						(r) => isAiRule(r) && r.action.subject.subject.target.subject === stackId,
					);
					const sessionId = rule?.filters.at(0)?.subject;
					if (typeof sessionId === "string") {
						return sessionId;
					}
				},
			},
		);
	}

	async fetchListWorkspaceRules(projectId: string) {
		return await this.backendApi.endpoints.listWorkspaceRules.fetch(
			{ projectId },
			{ transform: (result) => workspaceRulesSelectors.selectAll(result) },
		);
	}
}
