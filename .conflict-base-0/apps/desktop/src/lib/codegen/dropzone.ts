import { CommitDropData } from '$lib/commits/dropHandler';
import { FileChangeDropData, HunkDropDataV3 } from '$lib/dragging/draggables';
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
		private stackId: string | undefined,
		private rulesService: RulesService,
		private headerAlreadyHasRule: boolean
	) {}

	accepts(data: unknown): boolean {
		if (!this.stackId) return false;
		if (this.headerAlreadyHasRule) return false;
		if (!(data instanceof CodegenRuleDropData)) return false;
		if (data.rule.action.subject.subject.target.subject === this.stackId) return false;
		return true;
	}

	ondrop(data: CodegenRuleDropData): void {
		if (!this.stackId) {
			console.warn('Unsupported operation, no stackId provided.');
			return;
		}
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
		private stackId: string | undefined,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		if (!this.stackId) return false;
		return data instanceof CommitDropData && data.stackId === this.stackId;
	}

	ondrop(data: CommitDropData): void {
		if (!this.stackId) {
			console.warn('Unsupported operation, no stackId provided.');
			return;
		}
		this.add([{ type: 'commit', commitId: data.commit.id }]);
	}
}

export class CodegenFileDropHandler implements DropzoneHandler {
	constructor(
		private stackId: string | undefined,
		private branchName: string,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		if (!this.stackId) return false;
		return (
			data instanceof FileChangeDropData &&
			(data.stackId === undefined || data.stackId === this.stackId)
		);
	}

	async ondrop(data: FileChangeDropData): Promise<void> {
		if (!this.stackId) {
			console.warn('Unsupported operation, no stackId provided.');
			return;
		}
		const changes = await data.treeChanges();
		const commitId = data.selectionId.type === 'commit' ? data.selectionId.commitId : undefined;
		const attachments: PromptAttachment[] = changes.map((change) => ({
			type: 'file',
			branchName: this.branchName,
			path: change.path,
			commitId
		}));
		this.add(attachments);
	}
}

export class CodegenHunkDropHandler implements DropzoneHandler {
	constructor(
		private stackId: string | undefined,
		private add: (items: PromptAttachment[]) => void
	) {}

	accepts(data: unknown): boolean {
		if (!this.stackId) return false;
		return (
			data instanceof HunkDropDataV3 && (data.stackId === null || data.stackId === this.stackId)
		);
	}

	ondrop(data: HunkDropDataV3): void {
		if (!this.stackId) {
			console.warn('Unsupported operation, no stackId provided.');
			return;
		}
		this.add([
			{
				type: 'lines',
				path: data.change.path,
				start: data.hunk.newStart,
				end: data.hunk.newStart + data.hunk.newLines - 1,
				commitId: data.commitId
			}
		]);
	}
}
