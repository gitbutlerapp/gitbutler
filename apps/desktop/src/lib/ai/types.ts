import { isStr } from "@gitbutler/ui/utils/string";
import type { Persisted } from "@gitbutler/shared/persisted";

export enum ModelKind {
	OpenAI = "openai",
	Anthropic = "anthropic",
	Ollama = "ollama",
	LMStudio = "lmstudio",
}

// https://platform.openai.com/docs/models
export enum OpenAIModelName {
	GPT5 = "gpt-5.4",
	GPT5Mini = "gpt-5-mini",
	GPT5Nano = "gpt-5-nano",
}

// https://docs.anthropic.com/en/docs/about-claude/models/overview
export enum AnthropicModelName {
	Haiku = "claude-haiku-4-5-20251001",
	Sonnet = "claude-sonnet-4-6",
	Opus = "claude-opus-4-6",
}

export enum MessageRole {
	System = "system",
	User = "user",
	Assistant = "assistant",
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
