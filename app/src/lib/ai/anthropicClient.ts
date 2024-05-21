import { SHORT_DEFAULT_COMMIT_TEMPLATE, SHORT_DEFAULT_BRANCH_TEMPLATE } from '$lib/ai/prompts';
import { fetch, Body } from '@tauri-apps/api/http';
import type { AIClient, AnthropicModelName, PromptMessage } from '$lib/ai/types';

type AnthropicAPIResponse = { content: { text: string }[] };

export class AnthropicAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;

	constructor(
		private apiKey: string,
		private modelName: AnthropicModelName
	) {}

	async evaluate(prompt: PromptMessage[]) {
		const body = Body.json({
			messages: prompt,
			max_tokens: 1024,
			model: this.modelName
		});

		const response = await fetch<AnthropicAPIResponse>('https://api.anthropic.com/v1/messages', {
			method: 'POST',
			headers: {
				'x-api-key': this.apiKey,
				'anthropic-version': '2023-06-01',
				'content-type': 'application/json'
			},
			body
		});

		return response.data.content[0].text;
	}
}
