import { SHORT_DEFAULT_BRANCH_TEMPLATE, SHORT_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import { failure, ok, type Result } from '$lib/result';
import type { OpenAIModelName, Prompt, AIClient } from '$lib/ai/types';
import type OpenAI from 'openai';

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;

	constructor(
		private modelName: OpenAIModelName,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string>> {
		try {
			const response = await this.openAI.chat.completions.create({
				messages: prompt,
				model: this.modelName,
				max_tokens: 400
			});

			if (response.choices[0]?.message.content) {
				return ok(response.choices[0]?.message.content);
			} else {
				return failure('Open AI generated an empty message');
			}
		} catch (e) {
			if (e instanceof Error) {
				return failure(e.message);
			} else {
				return failure('Failed to contact Open AI');
			}
		}
	}
}
