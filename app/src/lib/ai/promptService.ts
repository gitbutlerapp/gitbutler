import { LONG_DEFAULT_BRANCH_TEMPLATE, LONG_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import { persisted, type Persisted } from '$lib/persisted/persisted';
import type { Prompts, UserPrompt } from '$lib/ai/types';

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

	selectedBranchPrompt(projectId: string): Persisted<UserPrompt | undefined> {
		return persisted<UserPrompt | undefined>(
			undefined,
			`${PromptPersistedKey.Branch}-${projectId}`
		);
	}

	selectedCommitPrompt(projectId: string): Persisted<UserPrompt | undefined> {
		return persisted<UserPrompt | undefined>(
			undefined,
			`${PromptPersistedKey.Commit}-${projectId}`
		);
	}
}
