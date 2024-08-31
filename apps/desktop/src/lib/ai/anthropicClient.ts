import { SHORT_DEFAULT_COMMIT_TEMPLATE, SHORT_DEFAULT_BRANCH_TEMPLATE } from '$lib/ai/prompts';
import { type AIClient, type AnthropicModelName, type Prompt } from '$lib/ai/types';
import { buildFailureFromAny, ok, type Result } from '$lib/result';
import { fetch } from '@tauri-apps/plugin-http';

type AnthropicAPIResponse = {
	content: { text: string }[];
	error: { type: string; message: string };
};

export class AnthropicAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;

	constructor(
		private apiKey: string,
		private modelName: AnthropicModelName
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
		const body = JSON.stringify({
			messages: prompt,
			max_tokens: 1024,
			model: this.modelName
		});

		const response = await fetch('https://api.anthropic.com/v1/messages', {
			method: 'POST',
			headers: {
				'x-api-key': this.apiKey,
				'anthropic-version': '2023-06-01',
				'content-type': 'application/json'
			},
			body
		});

		const data = (await response.json()) as AnthropicAPIResponse;
		if (response.ok) {
			return ok(data.content[0]?.text || '');
		} else {
			return buildFailureFromAny(
				`Anthropic returned error code ${response.status} ${data?.error?.message}`
			);
		}
	}
}
