import { type AIClient, type PromptMessage, MessageRole } from '$lib/backend/aiClient';
import { fetch, Body } from '@tauri-apps/api/http';
import type { AnthropicModelName } from '../types';

type AnthropicAPIResponse = { content: { text: string }[] };

export class AnthropicAIClient implements AIClient {
	constructor(
		private apiKey: string,
		private modelName: AnthropicModelName
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const body = Body.json({
			messages,
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
