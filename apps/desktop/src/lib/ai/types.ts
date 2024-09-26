import type { Persisted } from '$lib/persisted/persisted';
import type { Result } from '$lib/result';

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic',
	Ollama = 'ollama'
}

// https://platform.openai.com/docs/models
export enum OpenAIModelName {
	GPT35Turbo = 'gpt-3.5-turbo',
	GPT4 = 'gpt-4',
	GPT4Turbo = 'gpt-4-turbo',
	GPT4o = 'gpt-4o',
	GPT4oMini = 'gpt-4o-mini'
}

// https://docs.anthropic.com/en/docs/about-claude/models
export enum AnthropicModelName {
	Opus = 'claude-3-opus-20240229',
	Sonnet = 'claude-3-sonnet-20240229',
	Haiku = 'claude-3-haiku-20240307',
	Sonnet35 = 'claude-3-5-sonnet-20240620'
}

export enum MessageRole {
	System = 'system',
	User = 'user',
	Assistant = 'assistant'
}

export interface PromptMessage {
	content: string;
	role: MessageRole;
}

export type Prompt = PromptMessage[];

export interface AIClient {
	evaluate(prompt: Prompt): Promise<Result<string, Error>>;

	defaultBranchTemplate: Prompt;
	defaultCommitTemplate: Prompt;
	defaultPRTemplate: Prompt;
}

export type UserPrompt = {
	id: string;
	name: string;
	prompt: Prompt;
};

export interface Prompts {
	defaultPrompt: Prompt;
	userPrompts: Persisted<UserPrompt[]>;
}
