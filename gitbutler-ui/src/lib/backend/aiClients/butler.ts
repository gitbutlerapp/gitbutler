import { MessageRole, type AIClient, type PromptMessage } from '$lib/backend/aiClient';
import type { ModelKind } from '$lib/backend/aiService';
import type { getCloudApiClient, User } from '$lib/backend/cloud';

export class ButlerAIClient implements AIClient {
	constructor(
		private cloud: ReturnType<typeof getCloudApiClient>,
		private user: User,
		private modelKind: ModelKind
	) {}

	async evaluate(prompt: string) {
		const messages: PromptMessage[] = [{ role: MessageRole.User, content: prompt }];

		const response = await this.cloud.ai.evaluatePrompt(this.user.access_token, {
			messages,
			max_tokens: 400
		});

		return response.message;
	}
}
