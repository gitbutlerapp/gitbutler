import { messageParamToPrompt, splitPromptMessages } from './anthropicUtils';
import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { ModelKind, type AIClient, type Prompt } from '$lib/ai/types';
import { map, wrapAsync, type Result } from '$lib/result';
import type { HttpClient } from '@gitbutler/shared/httpClient';

function splitPromptMessagesIfNecessary(
	modelKind: ModelKind,
	prompt: Prompt
): [Prompt, string | undefined] {
	switch (modelKind) {
		case ModelKind.Anthropic: {
			const [messages, system] = splitPromptMessages(prompt);
			return [messageParamToPrompt(messages), system];
		}
		case ModelKind.OpenAI:
		case ModelKind.Ollama:
			return [prompt, undefined];
	}
}

export class ButlerAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	constructor(
		private cloud: HttpClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
		const [messages, system] = splitPromptMessagesIfNecessary(this.modelKind, prompt);
		const response = await wrapAsync<{ message: string }, Error>(
			async () =>
				await this.cloud.post<{ message: string }>('evaluate_prompt/predict.json', {
					body: {
						messages,
						system,
						max_tokens: 400,
						model_kind: this.modelKind
					},
					token: this.userToken
				})
		);

		return map(response, ({ message }) => message);
	}
}
