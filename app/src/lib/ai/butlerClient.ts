import { MessageRole, type ModelKind, type AIClient, type PromptMessage } from '$lib/aiTypes';
import type { CloudClient } from '$lib/backend/cloud';

export class ButlerAIClient implements AIClient {
	constructor(
		private cloud: CloudClient,
		private userToken: string,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.cloud.evaluateAIPrompt(this.userToken, {
			messages,
			max_tokens: 400,
			model_kind: this.modelKind
		});

		return response.message;
	}
}
