import { messageParamToPrompt, splitPromptMessages } from '$lib/ai/anthropicUtils';
import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { ModelKind, type AIClient, type AIEvalOptions, type Prompt } from '$lib/ai/types';
import { stringStreamGenerator } from '$lib/utils/promise';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

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
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<string> {
		const [messages, system] = splitPromptMessagesIfNecessary(this.modelKind, prompt);
		const response = await this.cloud.postRaw('ai/stream', {
			body: {
				messages,
				system,
				max_tokens: 3600,
				model_kind: this.modelKind
			}
		});

		const reader = response.body?.getReader();
		if (!reader) {
			return '';
		}

		const buffer: string[] = [];
		for await (const chunk of stringStreamGenerator(reader)) {
			options?.onToken?.(chunk);
			buffer.push(chunk);
		}

		return buffer.join('');
	}
}
