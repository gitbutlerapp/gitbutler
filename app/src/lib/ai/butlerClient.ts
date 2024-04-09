import { MessageRole, type ModelKind, type AIClient, type PromptMessage } from '$lib/ai/types';
import { RequestMethod, type CloudClient } from '$lib/backend/cloud';

export class ButlerAIClient implements AIClient {
	constructor(
		private cloud: CloudClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.cloud.makeRequest<{ message: string }>({
			path: 'evaluate_prompt/predict.json',
			method: RequestMethod.POST,
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
