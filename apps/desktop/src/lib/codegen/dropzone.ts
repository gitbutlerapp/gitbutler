import type { DropzoneHandler } from '$lib/dragging/handler';
import type { AiRule } from '$lib/rules/rule';
import type RulesService from '$lib/rules/rulesService.svelte';

export class CodegenRuleDropData {
	constructor(public rule: AiRule) {}
}

export class CodegenRuleDropHandler implements DropzoneHandler {
	constructor(
		private projectId: string,
		private stackId: string,
		private rulesService: RulesService,
		private headerAlreadyHasRule: boolean
	) {}

	accepts(data: unknown): boolean {
		if (this.headerAlreadyHasRule) return false;
		if (!(data instanceof CodegenRuleDropData)) return false;
		if (data.rule.action.subject.subject.target.subject === this.stackId) return false;
		return true;
	}

	ondrop(data: CodegenRuleDropData): void {
		this.rulesService.updateWorkspaceRuleMutate({
			projectId: this.projectId,
			request: {
				id: data.rule.id,
				enabled: null,
				action: {
					type: 'explicit',
					subject: {
						type: 'assign',
						subject: { target: { subject: this.stackId, type: 'stackId' } }
					}
				},
				filters: null,
				trigger: null
			}
		});
	}
}
