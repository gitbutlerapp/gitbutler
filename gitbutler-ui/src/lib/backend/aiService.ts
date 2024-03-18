import { AnthropicAIClient } from '$lib/backend/aiClients/anthropic';
import { ButlerAIClient } from '$lib/backend/aiClients/butler';
import { OpenAIClient } from '$lib/backend/aiClients/openAI';
import * as toasts from '$lib/utils/toasts';
import OpenAI from 'openai';
import type { AIClient } from './aiClient';
import type { getCloudApiClient } from './cloud';
import type { GitConfigService } from './gitConfigService';

const diffLengthLimit = 20000;

const defaultCommitTemplate = `
Please could you write a commit message for my changes.
Explain what were the changes and why the changes were done.
Focus the most important changes.
Use the present tense.
Use a semantic commit prefix.
Hard wrap lines at 72 characters.
Ensure the title is only 50 characters.
Do not start any lines with the hash symbol.
Only respond with the commit message.
%{brief_style}
%{emoji_style}

Here is my git diff:
%{diff}
`;

const defaultBranchTemplate = `
Please could you write a branch name for my changes.
A branch name represent a brief description of the changes in the diff (branch).
Branch names should contain no whitespace and instead use dashes to separate words.
Branch names should contain a maximum of 5 words.
Only respond with the branch name.

Here is my git diff:
%{diff}
`;

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic'
}

export enum KeyOption {
	BringYourOwn = 'bringYourOwn',
	ButlerAPI = 'butlerAPI'
}

export enum OpenAIModelName {
	GPT35Turbo = 'gpt-3.5-turbo',
	GPT4 = 'gpt-4',
	GPT4Turbo = 'gpt-4-turbo-preview'
}

export enum AnthropicModelName {
	Opus = 'claude-3-opus-20240229',
	Sonnet = 'claude-3-sonnet-20240229',
	Haiku = 'claude-3-haiku-20240307'
}

export enum GitAIConfigKey {
	ModelProvider = 'gitbutler.aiModelProvider',
	OpenAIKeyOption = 'gitbutler.aiOpenAIKeyOption',
	OpenAIModelName = 'gitbutler.aiOpenAIModelName',
	OpenAIKey = 'gitbutler.aiOpenAIKey',
	AnthropicKeyOption = 'gitbutler.aiAnthropicKeyOption',
	AnthropicModelName = 'gitbutler.aiAnthropicModelName',
	AnthropicKey = 'gitbutler.aiAnthropicKey'
}

type SummarizeCommitOpts = {
	diff: string;
	useEmojiStyle?: boolean;
	useBriefStyle?: boolean;
	commitTemplate?: string;
	userToken?: string;
};

type SummarizeBranchOpts = {
	diff: string;
	branchTemplate?: string;
	userToken?: string;
};

export class AIService {
	constructor(
		private gitConfig: GitConfigService,
		private cloud: ReturnType<typeof getCloudApiClient>
	) {}

	async configurationValid(userToken?: string) {
		const modelKind = await this.gitConfig.getWithDefault<ModelKind>(
			GitAIConfigKey.ModelProvider,
			ModelKind.OpenAI
		);
		const openAIKeyOption = await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.OpenAIKeyOption,
			KeyOption.ButlerAPI
		);
		const openAIKey = await this.gitConfig.get(GitAIConfigKey.OpenAIKey);
		const anthropicKeyOption = await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.AnthropicKeyOption,
			KeyOption.ButlerAPI
		);
		const anthropicKey = await this.gitConfig.get(GitAIConfigKey.AnthropicKey);

		if (
			(modelKind == ModelKind.OpenAI && openAIKeyOption == KeyOption.ButlerAPI) ||
			(modelKind == ModelKind.Anthropic && anthropicKeyOption == KeyOption.ButlerAPI)
		) {
			return Boolean(userToken);
		}

		return Boolean(
			(modelKind == ModelKind.OpenAI && openAIKey) ||
				(modelKind == ModelKind.Anthropic && anthropicKey)
		);
	}

	// This optionally returns a summarizer. There are a few conditions for how this may occur
	// Firstly, if the user has opted to use the GB API and isn't logged in, it will return undefined
	// Secondly, if the user has opted to bring their own key but hasn't provided one, it will return undefined
	async buildClient(userToken?: string): Promise<undefined | AIClient> {
		const modelKind = await this.gitConfig.getWithDefault<ModelKind>(
			GitAIConfigKey.ModelProvider,
			ModelKind.OpenAI
		);
		const openAIKeyOption = await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.OpenAIKeyOption,
			KeyOption.ButlerAPI
		);
		const anthropicKeyOption = await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.AnthropicKeyOption,
			KeyOption.ButlerAPI
		);

		if (
			(modelKind == ModelKind.OpenAI && openAIKeyOption == KeyOption.ButlerAPI) ||
			(modelKind == ModelKind.Anthropic && anthropicKeyOption == KeyOption.ButlerAPI)
		) {
			if (!userToken) {
				toasts.error("When using GitButler's API to summarize code, you must be logged in");
				return;
			}
			return new ButlerAIClient(this.cloud, userToken, modelKind);
		}

		if (modelKind == ModelKind.OpenAI) {
			const openAIModelName = await this.gitConfig.getWithDefault<OpenAIModelName>(
				GitAIConfigKey.OpenAIModelName,
				OpenAIModelName.GPT35Turbo
			);
			const openAIKey = await this.gitConfig.get(GitAIConfigKey.OpenAIKey);

			if (!openAIKey) {
				toasts.error(
					'When using OpenAI in a bring your own key configuration, you must provide a valid token'
				);
				return;
			}

			const openAI = new OpenAI({ apiKey: openAIKey, dangerouslyAllowBrowser: true });
			return new OpenAIClient(openAIModelName, openAI);
		}
		if (modelKind == ModelKind.Anthropic) {
			const anthropicModelName = await this.gitConfig.getWithDefault<AnthropicModelName>(
				GitAIConfigKey.AnthropicModelName,
				AnthropicModelName.Haiku
			);
			const anthropicKey = await this.gitConfig.get(GitAIConfigKey.AnthropicKey);

			if (!anthropicKey) {
				toasts.error(
					'When using Anthropic in a bring your own key configuration, you must provide a valid token'
				);
				return;
			}

			return new AnthropicAIClient(anthropicKey, anthropicModelName);
		}
	}

	async summarizeCommit({
		diff,
		useEmojiStyle = false,
		useBriefStyle = false,
		commitTemplate = defaultCommitTemplate,
		userToken
	}: SummarizeCommitOpts) {
		const aiClient = await this.buildClient(userToken);
		if (!aiClient) return;

		let prompt = commitTemplate.replaceAll('%{diff}', diff.slice(0, diffLengthLimit));

		const briefPart = useBriefStyle
			? 'The commit message must be only one sentence and as short as possible.'
			: '';
		prompt = prompt.replaceAll('%{brief_style}', briefPart);

		const emojiPart = useEmojiStyle
			? 'Make use of GitMoji in the title prefix.'
			: "Don't use any emoji.";
		prompt = prompt.replaceAll('%{emoji_style}', emojiPart);

		let message = await aiClient.evaluate(prompt);

		if (useBriefStyle) {
			message = message.split('\n')[0];
		}

		const [summary, description] = message.split(/\n+(.*)/s);
		return description ? `${summary}\n\n${description}` : summary;
	}

	async summarizeBranch({
		diff,
		branchTemplate = defaultBranchTemplate,
		userToken = undefined
	}: SummarizeBranchOpts) {
		const aiClient = await this.buildClient(userToken);
		if (!aiClient) return;

		const prompt = branchTemplate.replaceAll('%{diff}', diff.slice(0, diffLengthLimit));
		const message = await aiClient.evaluate(prompt);
		return message.replaceAll(' ', '-').replaceAll('\n', '-');
	}
}
