import type { User, getCloudApiClient } from '$lib/backend/cloud';

enum ButlerMessageRole {
	User = 'user',
	System = 'system'
}

export interface ButlerPromptMessage {
	content: string;
	role: ButlerMessageRole;
}

export interface AIProvider {
	evaluate(prompt: string): Promise<string>;
}

export class ButlerAIProvider implements AIProvider {
	constructor(
		private cloud: ReturnType<typeof getCloudApiClient>,
		private user: User
	) {}

	async evaluate(prompt: string) {
		const messages: ButlerPromptMessage[] = [{ role: ButlerMessageRole.User, content: prompt }];

		const response = await this.cloud.ai.evaluatePrompt(this.user.access_token, { messages });

		return response.message;
	}
}
