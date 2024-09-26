import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { andThen, buildFailureFromAny, ok, wrapAsync, type Result } from '$lib/result';
import type { OpenAIModelName, Prompt, AIClient } from '$lib/ai/types';
import type OpenAI from 'openai';
import type { ChatCompletion } from 'openai/resources/index.mjs';

export class OpenAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	constructor(
		private modelName: OpenAIModelName,
		private openAI: OpenAI
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
		const responseResult = await wrapAsync<ChatCompletion, Error>(async () => {
			return await this.openAI.chat.completions.create({
				messages: prompt,
				model: this.modelName,
				max_tokens: 400
			});
		});

		return andThen(responseResult, (response) => {
			if (response.choices[0]?.message.content) {
				return ok(response.choices[0]?.message.content);
			} else {
				return buildFailureFromAny('Open AI generated an empty message');
			}
		});
	}
}
