import { MessageRole, type ModelKind, type AIClient, type PromptMessage } from '$lib/ai/types';
import type { HttpClient } from '$lib/backend/httpClient';

export class ButlerAIClient implements AIClient {
	constructor(
		private cloud: HttpClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.cloud.post<{ message: string }>({
			path: 'evaluate_prompt/predict.json',
			body: {
				messages,
				max_tokens: 400,
				model_kind: this.modelKind
			},
			token: this.userToken
		});

		return response.message;
	}
}
