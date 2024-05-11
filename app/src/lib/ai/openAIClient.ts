import {
	MessageRole,
	type OpenAIModelName,
	type PromptMessage,
	type AIClient
} from '$lib/ai/types';
import type OpenAI from 'openai';

export class OpenAIClient implements AIClient {
	constructor(
		private modelName: OpenAIModelName,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.openAI.chat.completions.create({
			messages,
			model: this.modelName,
			max_tokens: 400
		});

		return response.choices[0].message.content || '';
	}
}
