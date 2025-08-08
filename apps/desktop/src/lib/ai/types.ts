import { isStr } from '@gitbutler/ui/utils/string';
import type { Persisted } from '@gitbutler/shared/persisted';

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic',
	Ollama = 'ollama',
	LMStudio = 'lmstudio'
}

// https://platform.openai.com/docs/models
export enum OpenAIModelName {
	O3mini = 'o3-mini',
	O1mini = 'o1-mini',
	GPT5 = 'gpt-5',
	GPT5Mini = 'gpt-5-mini',
	GPT4_1 = 'gpt-4.1',
	GPT4_1Mini = 'gpt-4.1-mini',
	GPT4oMini = 'gpt-4o-mini'
}

// https://docs.anthropic.com/en/docs/about-claude/models/overview
export enum AnthropicModelName {
	Haiku = 'claude-3-5-haiku-20241022',
	Sonnet35 = 'claude-3-5-sonnet-20241022',
	Sonnet37 = 'claude-3-7-sonnet-20250219',
	Sonnet4 = 'claude-sonnet-4-0',
	Opus4 = 'claude-opus-4-0'
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

export type FileChange = {
	path: string;
	diffs: string[];
};
