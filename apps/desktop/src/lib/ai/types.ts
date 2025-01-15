import { isStr } from '@gitbutler/ui/utils/string';
import type { Result } from '$lib/ai/result';
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
	evaluate(prompt: Prompt, options?: AIEvalOptions): Promise<Result<string, Error>>;

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
