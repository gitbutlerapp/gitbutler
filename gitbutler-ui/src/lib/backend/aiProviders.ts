import { Body, fetch } from '@tauri-apps/api/http';
import type { User, getCloudApiClient } from '$lib/backend/cloud';
import type { AnthropicModel, ModelKind, OpenAIModel } from '$lib/backend/summarizerSettings';
import type OpenAI from 'openai';

enum MessageRole {
	User = 'user',
	Assistant = 'assisstant'
}

export interface PromptMessage {
	content: string;
	role: MessageRole;
}

export interface AIProvider {
	evaluate(prompt: string): Promise<string>;
}

export class ButlerAIProvider implements AIProvider {
	constructor(
		private cloud: ReturnType<typeof getCloudApiClient>,
		private user: User,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.cloud.ai.evaluatePrompt(this.user.access_token, {
			messages,
			max_tokens: 400
		});

		return response.message;
	}
}

export class OpenAIProvider implements AIProvider {
	constructor(
		private model: OpenAIModel,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.openAI.chat.completions.create({
			// @ts-expect-error There is a type mismatch where it seems to want a "name" paramater that isn't required https://github.com/openai/openai-openapi/issues/118#issuecomment-1847667988
			messages,
			model: this.model,
			max_tokens: 400
		});

		return response.choices[0].message.content || '';
	}
}

type AnthropicAPIResponse = { content: { text: string }[] };

export class AnthropicAIProvider implements AIProvider {
	constructor(
		private apiKey: string,
		private model: AnthropicModel
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const body = Body.json({
			messages,
			max_tokens: 1024,
			model: this.model
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
