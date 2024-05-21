import { SHORT_DEFAULT_BRANCH_TEMPLATE, SHORT_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import type { OpenAIModelName, PromptMessage, AIClient } from '$lib/ai/types';
import type OpenAI from 'openai';

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;

	constructor(
		private modelName: OpenAIModelName,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: PromptMessage[]) {
		const response = await this.openAI.chat.completions.create({
			messages: prompt,
			model: this.modelName,
			max_tokens: 400
		});

		return response.choices[0].message.content || '';
	}
}
