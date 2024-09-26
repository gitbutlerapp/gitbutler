import {
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { type AIClient, type AnthropicModelName, type Prompt } from '$lib/ai/types';
import { buildFailureFromAny, ok, type Result } from '$lib/result';
import { fetch, Body } from '@tauri-apps/api/http';

type AnthropicAPIResponse = {
	content: { text: string }[];
	error: { type: string; message: string };
};

export class AnthropicAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	constructor(
		private apiKey: string,
		private modelName: AnthropicModelName
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
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

		if (response.ok && response.data?.content?.[0]?.text) {
			return ok(response.data.content[0].text);
		} else {
			return buildFailureFromAny(
				`Anthropic returned error code ${response.status} ${response.data?.error?.message}`
			);
		}
	}
}
