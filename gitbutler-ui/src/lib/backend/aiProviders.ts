import { Body, fetch } from '@tauri-apps/api/http';
import OpenAI from 'openai';
import type { User, getCloudApiClient } from '$lib/backend/cloud';

enum MessageRole {
	User = 'user',
	Assistant = 'assisstant'
}

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic'
}

export enum KeyOption {
	BringYourOwn = 'bringYourOwn',
	ButlerAPI = 'butlerAPI'
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
	private openAI: OpenAI;

	constructor(apiKey: string) {
		this.openAI = new OpenAI({ apiKey, dangerouslyAllowBrowser: true });
	}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.openAI.chat.completions.create({
			// @ts-expect-error There is a type mismatch where it seems to want a "name" paramater that isn't required https://github.com/openai/openai-openapi/issues/118#issuecomment-1847667988
			messages,
			model: 'gpt-3.5-turbo',
			max_tokens: 400
		});

		return response.choices[0].message.content || '';
	}
}

type AnthropicAPIResponse = { content: { text: string }[] };

export class AnthropicAIProvider implements AIProvider {
	constructor(private apiKey: string) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const body = Body.json({
			messages,
			max_tokens: 1024,
			model: 'claude-3-opus-20240229'
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
