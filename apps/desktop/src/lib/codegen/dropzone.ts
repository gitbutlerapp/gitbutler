import { CommitDropData } from '$lib/commits/dropHandler';
import { ChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
import type { PromptAttachment } from '$lib/codegen/types';
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

export class CodegenCommitDropHandler implements DropzoneHandler {
	constructor(
		private stackId: string,
		private branchName: string,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		// For now, accept all commits without restrictions
		return data instanceof CommitDropData && data.stackId === this.stackId;
	}

	ondrop(data: CommitDropData): void {
		this.add([{ type: 'commit', branchName: this.branchName, commitId: data.commit.id }]);
	}
}

export class CodegenFileDropHandler implements DropzoneHandler {
	constructor(
		private stackId: string,
		private branchName: string,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof ChangeDropData &&
			(data.stackId === undefined || data.stackId === this.stackId)
		);
	}

	ondrop(data: ChangeDropData): void {
		this.add([{ type: 'file', branchName: this.branchName, path: data.change.path }]);
	}
}

export class CodegenHunkDropHandler implements DropzoneHandler {
	constructor(
		private stackId: string,
		private branchName: string,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		// For now, accept all hunks without restrictions
		return (
			data instanceof HunkDropDataV3 && (data.stackId === null || data.stackId === this.stackId)
		);
	}

	ondrop(data: HunkDropDataV3): void {
		this.add([
			{
				type: 'lines',
				branchName: this.branchName,
				path: data.change.path,
				start: data.hunk.newStart,
				end: data.hunk.newStart + data.hunk.newLines - 1
			}
		]);
	}
}
