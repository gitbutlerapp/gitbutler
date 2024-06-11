import type { Persisted } from '$lib/persisted/persisted';
import type { Branded } from '$lib/utils/branding';

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic',
	Ollama = 'ollama'
}

export enum OpenAIModelName {
	GPT35Turbo = 'gpt-3.5-turbo',
	GPT4 = 'gpt-4',
	GPT4Turbo = 'gpt-4-turbo',
	GPT4o = 'gpt-4o'
}

export enum AnthropicModelName {
	Opus = 'claude-3-opus-20240229',
	Sonnet = 'claude-3-sonnet-20240229',
	Haiku = 'claude-3-haiku-20240307'
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

export type CustomPromptDirective = Branded<string, 'CustomPromptDirective'>;

export enum PromptDirective {
	WriteCommitMessage = 'Please could you write a commit message for my changes.',
	ImproveCommitMessage = "Please complete the commit message leaving the existing commit MESSAGE as is. DON't change the existing MESSAGE, just add to it.",
	CommitMessageBrevity = 'The commit message must be only one sentence and as short as possible.',
	CommitMessageUseEmoji = 'Use emoji in the title prefix. Add it if not present.',
	CommitMessageDontUseEmoji = "Don't use any emoji.",
	WriteBranchName = 'Please could you write a branch name for my changes.'
}

export enum PromptTemplateParam {
	CreateOrRewriteMessage = '%{create_or_rewrite_message}',
	ExistingMessage = '%{existing_message}',
	Diff = '%{diff}',
	BriefStyle = '%{brief_style}',
	EmojiStyle = '%{emoji_style}'
}

export enum InternalPromptMessageType {
	Example = 'example',
	MainPrompt = 'main-prompt'
}
interface InternalPromptMessageExample extends PromptMessage {
	type: InternalPromptMessageType.Example;
	forDirective: PromptDirective;
}

interface InternalPromptMessageMainPrompt extends PromptMessage {
	type: InternalPromptMessageType.MainPrompt;
}

export type InternalPromptMessage = InternalPromptMessageExample | InternalPromptMessageMainPrompt;

export type InternalPrompt = InternalPromptMessage[];

export function isInternalPromptMessageExample(
	message: PromptMessage
): message is InternalPromptMessageExample {
	return (message as InternalPromptMessage).type === InternalPromptMessageType.Example;
}

export interface AIClient {
	evaluate(prompt: Prompt): Promise<string>;

	defaultBranchTemplate: Prompt;
	defaultCommitTemplate: InternalPrompt;
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
