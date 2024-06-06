import {
	LONG_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	LONG_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE
} from '$lib/ai/prompts';
import { persisted, type Persisted } from '$lib/persisted/persisted';
import { get } from 'svelte/store';
import type { Prompt, Prompts, UserPrompt } from '$lib/ai/types';

enum PromptPersistedKey {
	Branch = 'aiBranchPrompts',
	Commit = 'aiCommitPrompts'
}

export class PromptService {
	get branchPrompts(): Prompts {
		return {
			defaultPrompt: LONG_DEFAULT_BRANCH_TEMPLATE,
			userPrompts: persisted<UserPrompt[]>([], PromptPersistedKey.Branch)
		};
	}

	get commitPrompts(): Prompts {
		return {
			defaultPrompt: LONG_DEFAULT_COMMIT_TEMPLATE,
			userPrompts: persisted<UserPrompt[]>([], PromptPersistedKey.Commit)
		};
	}

	selectedBranchPromptId(projectId: string): Persisted<string | undefined> {
		return persisted<string | undefined>(undefined, `${PromptPersistedKey.Branch}-${projectId}`);
	}

	selectedBranchPrompt(projectId: string): Prompt | undefined {
		const id = get(this.selectedBranchPromptId(projectId));

		if (!id) return;

		return this.findPrompt(get(this.branchPrompts.userPrompts), id);
	}

	selectedCommitPromptId(projectId: string): Persisted<string | undefined> {
		return persisted<string | undefined>(undefined, `${PromptPersistedKey.Commit}-${projectId}`);
	}

	selectedCommitPrompt(projectId: string): Prompt | undefined {
		const id = get(this.selectedCommitPromptId(projectId));

		if (!id) return;

		return this.findPrompt(get(this.commitPrompts.userPrompts), id);
	}

	findPrompt(prompts: UserPrompt[], promptId: string) {
		const prompt = prompts.find((userPrompt) => userPrompt.id === promptId)?.prompt;

		if (!prompt) return;
		if (this.promptMissingContent(prompt)) return;

		return prompt;
	}

	promptEquals(prompt1: Prompt, prompt2: Prompt) {
		if (prompt1.length !== prompt2.length) return false;

		for (const indexPromptMessage of prompt1.entries()) {
			const [index, promptMessage] = indexPromptMessage;

			if (
				promptMessage.role !== prompt2[index].role ||
				promptMessage.content !== prompt2[index].content
			) {
				return false;
			}
		}

		return true;
	}

	promptMissingContent(prompt: Prompt) {
		for (const promptMessage of prompt) {
			if (!promptMessage.content) return true;
		}

		return false;
	}

	createDefaultUserPrompt(type: 'commits' | 'branches'): UserPrompt {
		return {
			id: crypto.randomUUID(),
			name: 'My Prompt',
			prompt: type === 'branches' ? SHORT_DEFAULT_BRANCH_TEMPLATE : SHORT_DEFAULT_COMMIT_TEMPLATE
		};
	}
}
