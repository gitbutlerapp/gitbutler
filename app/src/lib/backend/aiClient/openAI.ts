import { type AIClient, type PromptMessage, MessageRole } from '$lib/backend/aiClient';
import type { OpenAIModelName } from '../types';
import type OpenAI from 'openai';

export class OpenAIClient implements AIClient {
	constructor(
		private modelName: OpenAIModelName,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.openAI.chat.completions.create({
			// @ts-expect-error There is a type mismatch where it seems to want a "name" paramater
			// that isn't required https://github.com/openai/openai-openapi/issues/118#issuecomment-1847667988
			messages,
			model: this.modelName,
			max_tokens: 400
		});

		return response.choices[0].message.content || '';
	}
}
