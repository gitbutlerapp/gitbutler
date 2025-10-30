import { showError, showToast } from '$lib/notifications/toasts';
import type DiffInputContext from '$lib/ai/diffInputContext.svelte';
import type { PromptService } from '$lib/ai/promptService';
import type { AIService, DiffInput } from '$lib/ai/service';

type GenerateCommitMessageParams = {
	branchName?: string;
	diffInput?: DiffInput[];
	onToken?: (token: string) => void;
	useHaiku?: boolean;
	useEmojiStyle?: boolean;
	useBriefStyle?: boolean;
};

export default class AIMacros {
	private _canUseAI: boolean = $state<boolean>(false);

	constructor(
		private readonly projectId: string,
		private readonly aiService: AIService,
		private readonly promptService: PromptService,
		private readonly diffInputContext: DiffInputContext
	) {}

	async setGenAIEnabled(enabled: boolean) {
		// TODO: Should this be called here, or evertime that we check the canUseAI?
		const aiConfigurartionValid = await this.aiService.validateConfiguration();
		this._canUseAI = enabled && aiConfigurartionValid;
	}

	get canUseAI() {
		return this._canUseAI;
	}

	/**
	 * Generate a commit message based on the selected changes.
	 *
	 * If AI is not enabled, this will return undefined.
	 */
	async generateCommitMessage(params: GenerateCommitMessageParams): Promise<string | undefined> {
		if (!this.canUseAI) return;

		const prompt = this.promptService.selectedCommitPrompt(this.projectId);
		const diffInput = params.diffInput ?? (await this.diffInputContext.diffInput());
		if (!diffInput) {
			showError('Failed to generate commit message', 'No changes found');
			return;
		}

		const output = await this.aiService.summarizeCommit({
			diffInput,
			useHaiku: params.useHaiku ?? false,
			useEmojiStyle: params.useEmojiStyle ?? false,
			useBriefStyle: params.useBriefStyle ?? false,
			commitTemplate: prompt,
			branchName: params.branchName,
			onToken: params.onToken
		});

		return output;
	}

	/**
	 * Generate a branch name based on the selected changes.
	 *
	 * If AI is not enabled, this will return undefined.
	 */
	async generateBranchNameFromDiffInput(diffInput: DiffInput[]): Promise<string | undefined> {
		if (!this.canUseAI) return;

		const prompt = this.promptService.selectedBranchPrompt(this.projectId);
		const newBranchName = await this.aiService.summarizeBranch({
			type: 'hunks',
			hunks: diffInput,
			branchTemplate: prompt
		});

		return newBranchName;
	}

	/**
	 * Create a new branch name and commit message based on the current diff input.
	 *
	 * If AI is not enabled, this will return undefined for both branch name and commit message.
	 */
	async getBranchNameAndCommitMessage(): Promise<{
		branchName: string | undefined;
		commitMessage: string | undefined;
	}> {
		if (!this.canUseAI) return { branchName: undefined, commitMessage: undefined };

		const diffInput = await this.diffInputContext.diffInput();
		if (!diffInput) {
			showError('Failed to generate branch name', 'No changes found');
			return { branchName: undefined, commitMessage: undefined };
		}
		const branchName = await this.generateBranchNameFromDiffInput(diffInput);

		if (!branchName) {
			showToast({
				style: 'error',
				message: 'Failed to generate branch name.'
			});
			return { branchName, commitMessage: undefined };
		}

		const commitMessage = await this.generateCommitMessage({ branchName, diffInput });

		if (!commitMessage) {
			showToast({
				style: 'error',
				message: 'Failed to generate commit message.'
			});
			return { branchName, commitMessage };
		}

		return { branchName, commitMessage };
	}
}
