import { SHORT_DEFAULT_BRANCH_TEMPLATE, SHORT_DEFAULT_COMMIT_TEMPLATE } from '$lib/ai/prompts';
import { err, ok, type Result } from '$lib/result';
import type { AIClient, ModelKind, Prompt } from '$lib/ai/types';
import type { HttpClient } from '$lib/backend/httpClient';

export class ButlerAIClient implements AIClient {
	defaultCommitTemplate = SHORT_DEFAULT_COMMIT_TEMPLATE;
	defaultBranchTemplate = SHORT_DEFAULT_BRANCH_TEMPLATE;

	constructor(
		private cloud: HttpClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: Prompt): Promise<Result<string, string>> {
		try {
			const response = await this.cloud.post<{ message: string }>('evaluate_prompt/predict.json', {
				body: {
					messages: prompt,
					max_tokens: 400,
					model_kind: this.modelKind
				},
				token: this.userToken
			});

			return ok(response.message);
		} catch (e) {
			if (e instanceof Error) {
				return err(e.message);
			} else {
				return err('Failed to contant GitButler API');
			}
		}
	}
}
