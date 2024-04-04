export enum MessageRole {
	User = 'user',
	Assistant = 'assisstant'
}

export interface PromptMessage {
	content: string;
	role: MessageRole;
}

export interface AIClient {
	evaluate(prompt: string): Promise<string>;
}
