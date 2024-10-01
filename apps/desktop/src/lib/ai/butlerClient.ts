import {
	SHORT_DEFAULT_BRANCH_TEMPLATE,
	SHORT_DEFAULT_COMMIT_TEMPLATE,
	SHORT_DEFAULT_PR_TEMPLATE
} from '$lib/ai/prompts';
import { map, type Result } from '$lib/result';
import type { AIClient, ModelKind, Prompt } from '$lib/ai/types';
import type { HttpClient } from '$lib/backend/httpClient';

export class ButlerAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;
	defaultPRTemplate = SHORT_DEFAULT_PR_TEMPLATE;

	constructor(
		private cloud: HttpClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, Error>> {
		const response = await this.cloud.postSafe<{ message: string }>(
			'evaluate_prompt/predict.json',
			{
				body: {
					messages: prompt,
					max_tokens: 400,
					model_kind: this.modelKind
				},
				token: this.userToken
			}
		);

		return map(response, ({ message }) => message);
	}
}
