import BackendService, { workspaceRulesSelectors } from '$lib/backend/backendService.svelte';
import type { BackendApi } from '$lib/state/clientState.svelte';

export default class RulesService {
	private backendService: BackendService;
	private apis: ReturnType<typeof this.backendService.get>;

	constructor(backendApi: BackendApi) {
		this.backendService = BackendService.getInstance(backendApi);
		this.apis = this.backendService.get();
	}

	get createWorkspaceRule() {
		return this.apis.createWorkspaceRuleUseMutation();
	}

	get deleteWorkspaceRule() {
		return this.apis.deleteWorkspaceRuleUseMutation();
	}

	get updateWorkspaceRule() {
		return this.apis.updateWorkspaceRuleUseMutation();
	}

	listWorkspaceRules(projectId: string) {
		return this.apis.listWorkspaceRulesUseQuery(
			{ projectId },
			{ transform: (result) => workspaceRulesSelectors.selectAll(result) }
		);
	}
}
