import { isStr } from '@gitbutler/ui/utils/string';
import type { Persisted } from '@gitbutler/shared/persisted';

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
	Haiku = 'claude-3-5-haiku-latest',
	Sonnet35 = 'claude-3-5-sonnet-latest',
	Sonnet37 = 'claude-3-7-sonnet-latest'
}

export enum MessageRole {
	System = 'system',
	User = 'user',
	Assistant = 'assistant'
}

export function isMessageRole(role: unknown): role is MessageRole {
	if (!isStr(role)) return false;
	const roles = Object.values(MessageRole);
	return roles.includes(role as MessageRole);
}

export interface PromptMessage {
	content: string;
	role: MessageRole;
}

export type Prompt = PromptMessage[];

export interface AIEvalOptions {
	maxTokens?: number;
	onToken?: (t: string) => void;
}

export interface AIClient {
	evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<string>;

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
