import { InjectionToken } from '@gitbutler/core/context';
import { stringStreamGenerator } from '@gitbutler/shared/utils/promise';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

export enum MessageRole {
	System = 'system',
	User = 'user',
	Assistant = 'assistant'
}

export interface PromptMessage {
	content: string;
	role: MessageRole;
}

export type Prompt = PromptMessage[];

export const BUTLER_AI_CLIENT = new InjectionToken<ButlerAIClient>('ButlerAIClient');

export class ButlerAIClient {
	constructor(private cloud: HttpClient) {}

	async evaluate(
		systemPrompt: string,
		prompt: Prompt,
		onToken: (t: string) => void
	): Promise<string> {
		const response = await this.cloud.postRaw('ai/stream', {
			body: {
				messages: prompt,
				system: systemPrompt,
				max_tokens: 3600,
				model_kind: 'openai'
			}
		});

		const reader = response.body?.getReader();
		if (!reader) {
			return '';
		}

		const buffer: string[] = [];
		for await (const chunk of stringStreamGenerator(reader)) {
			onToken(chunk);
			buffer.push(chunk);
		}

		return buffer.join('');
	}
}
