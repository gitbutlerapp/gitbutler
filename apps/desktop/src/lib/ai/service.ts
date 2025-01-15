import { DEFAULT_PR_SUMMARY_MAIN_DIRECTIVE, getPrTemplateDirective } from './prompts';
import {
	OpenAIModelName,
	type AIClient,
	AnthropicModelName,
	ModelKind,
	MessageRole,
	type Prompt
} from './types';
import { AnthropicAIClient } from '$lib/ai/anthropicClient';
import { ButlerAIClient } from '$lib/ai/butlerClient';
import {
	DEFAULT_OLLAMA_ENDPOINT,
	DEFAULT_OLLAMA_MODEL_NAME,
	OllamaClient
} from '$lib/ai/ollamaClient';
import { OpenAIClient } from '$lib/ai/openAIClient';
import { buildFailureFromAny, isFailure, ok, type Result } from '$lib/ai/result';
import { splitMessage } from '$lib/utils/commitMessage';
import { get } from 'svelte/store';
import type { GitConfigService } from '$lib/config/gitConfigService';
import type { SecretsService } from '$lib/secrets/secretsService';
import type { TokenMemoryService } from '$lib/stores/tokenMemoryService';
import type { HttpClient } from '@gitbutler/shared/network/httpClient';

const maxDiffLengthLimitForAPI = 5000;
const prDescriptionTokenLimit = 4096;

export enum KeyOption {
	BringYourOwn = 'bringYourOwn',
	ButlerAPI = 'butlerAPI'
}

export enum AISecretHandle {
	OpenAIKey = 'aiOpenAIKey',
	AnthropicKey = 'aiAnthropicKey'
}

export enum GitAIConfigKey {
	ModelProvider = 'gitbutler.aiModelProvider',
	OpenAIKeyOption = 'gitbutler.aiOpenAIKeyOption',
	OpenAIModelName = 'gitbutler.aiOpenAIModelName',
	AnthropicKeyOption = 'gitbutler.aiAnthropicKeyOption',
	AnthropicModelName = 'gitbutler.aiAnthropicModelName',
	DiffLengthLimit = 'gitbutler.diffLengthLimit',
	OllamaEndpoint = 'gitbutler.aiOllamaEndpoint',
	OllamaModelName = 'gitbutler.aiOllamaModelName'
}

interface BaseAIServiceOpts {
	userToken?: string;
	onToken?: (token: string) => void;
}

interface SummarizeCommitOpts extends BaseAIServiceOpts {
	diffInput: DiffInput[];
	useEmojiStyle?: boolean;
	useBriefStyle?: boolean;
	commitTemplate?: Prompt;
	branchName?: string;
}

interface SummarizeBranchOpts extends BaseAIServiceOpts {
	hunks: DiffInput[];
	branchTemplate?: Prompt;
}

interface SummarizePROpts extends BaseAIServiceOpts {
	commitMessages: string[];
	title: string;
	body: string;
	directive?: string;
	prTemplate?: Prompt;
	prBodyTemplate?: string;
}

export interface DiffInput {
	filePath: string;
	diff: string;
}

// Exported for testing only
export function buildDiff(hunks: DiffInput[], limit: number) {
	return shuffle(hunks.map((h) => `${h.filePath} - ${h.diff}`))
		.join('\n')
		.slice(0, limit);
}

function shuffle<T>(items: T[]): T[] {
	return items
		.map((item) => ({ item, value: Math.random() }))
		.sort(({ value: a }, { value: b }) => a - b)
		.map((item) => item.item);
}

export class AIService {
	prSummaryMainDirective: Readonly<string> = DEFAULT_PR_SUMMARY_MAIN_DIRECTIVE;

	constructor(
		private gitConfig: GitConfigService,
		private secretsService: SecretsService,
		private cloud: HttpClient,
		private tokenMemoryService: TokenMemoryService
	) {}

	async getModelKind() {
		return await this.gitConfig.getWithDefault<ModelKind>(
			GitAIConfigKey.ModelProvider,
			ModelKind.OpenAI
		);
	}

	async getOpenAIKeyOption() {
		return await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.OpenAIKeyOption,
			KeyOption.ButlerAPI
		);
	}

	async getOpenAIKey() {
		return await this.secretsService.get(AISecretHandle.OpenAIKey);
	}

	async getOpenAIModleName() {
		return await this.gitConfig.getWithDefault<OpenAIModelName>(
			GitAIConfigKey.OpenAIModelName,
			OpenAIModelName.GPT35Turbo
		);
	}

	async getAnthropicKeyOption() {
		return await this.gitConfig.getWithDefault<KeyOption>(
			GitAIConfigKey.AnthropicKeyOption,
			KeyOption.ButlerAPI
		);
	}

	async getAnthropicKey() {
		return await this.secretsService.get(AISecretHandle.AnthropicKey);
	}

	async getAnthropicModelName() {
		return await this.gitConfig.getWithDefault<AnthropicModelName>(
			GitAIConfigKey.AnthropicModelName,
			AnthropicModelName.Haiku
		);
	}

	async getDiffLengthLimit() {
		const limitString = await this.gitConfig.getWithDefault<string>(
			GitAIConfigKey.DiffLengthLimit,
			'5000'
		);

		return parseInt(limitString, 10);
	}

	/**
	 * Returns the diff length limit with a specificed upper bound of characers in order to not inundate the API.
	 */
	async getDiffLengthLimitConsideringAPI() {
		const diffLengthLimit = await this.getDiffLengthLimit();

		if (await this.usingGitButlerAPI()) {
			return Math.max(maxDiffLengthLimitForAPI, diffLengthLimit);
		} else {
			return diffLengthLimit;
		}
	}

	async getOllamaEndpoint() {
		return await this.gitConfig.getWithDefault<string>(
			GitAIConfigKey.OllamaEndpoint,
			DEFAULT_OLLAMA_ENDPOINT
		);
	}

	async getOllamaModelName() {
		return await this.gitConfig.getWithDefault<string>(
			GitAIConfigKey.OllamaModelName,
			DEFAULT_OLLAMA_MODEL_NAME
		);
	}

	async usingGitButlerAPI() {
		const modelKind = await this.getModelKind();
		const openAIKeyOption = await this.getOpenAIKeyOption();
		const anthropicKeyOption = await this.getAnthropicKeyOption();

		const openAIActiveAndUsingButlerAPI =
			modelKind === ModelKind.OpenAI && openAIKeyOption === KeyOption.ButlerAPI;
		const anthropicActiveAndUsingButlerAPI =
			modelKind === ModelKind.Anthropic && anthropicKeyOption === KeyOption.ButlerAPI;

		return openAIActiveAndUsingButlerAPI || anthropicActiveAndUsingButlerAPI;
	}

	async validateConfiguration(): Promise<boolean> {
		const modelKind = await this.getModelKind();
		const openAIKey = await this.getOpenAIKey();
		const anthropicKey = await this.getAnthropicKey();
		const ollamaEndpoint = await this.getOllamaEndpoint();
		const ollamaModelName = await this.getOllamaModelName();

		if (await this.usingGitButlerAPI()) return !!get(this.tokenMemoryService.token);

		const openAIActiveAndKeyProvided = modelKind === ModelKind.OpenAI && !!openAIKey;
		const anthropicActiveAndKeyProvided = modelKind === ModelKind.Anthropic && !!anthropicKey;
		const ollamaActiveAndEndpointProvided =
			modelKind === ModelKind.Ollama && !!ollamaEndpoint && !!ollamaModelName;

		return (
			openAIActiveAndKeyProvided || anthropicActiveAndKeyProvided || ollamaActiveAndEndpointProvided
		);
	}

	// This optionally returns a summarizer. There are a few conditions for how this may occur
	// Firstly, if the user has opted to use the GB API and isn't logged in, it will return undefined
	// Secondly, if the user has opted to bring their own key but hasn't provided one, it will return undefined
	async buildClient(): Promise<Result<AIClient, Error>> {
		const modelKind = await this.getModelKind();

		if (await this.usingGitButlerAPI()) {
			// TODO(CTO): Once @estib has landed the new auth, it would be good to
			// about a good way of checking whether the user is authenticated.
			if (!get(this.tokenMemoryService.token)) {
				return buildFailureFromAny(
					"When using GitButler's API to summarize code, you must be logged in"
				);
			}

			return ok(new ButlerAIClient(this.cloud, modelKind));
		}

		if (modelKind === ModelKind.Ollama) {
			const ollamaEndpoint = await this.getOllamaEndpoint();
			const ollamaModelName = await this.getOllamaModelName();
			return ok(new OllamaClient(ollamaEndpoint, ollamaModelName));
		}

		if (modelKind === ModelKind.OpenAI) {
			const openAIModelName = await this.getOpenAIModleName();
			const openAIKey = await this.getOpenAIKey();

			if (!openAIKey) {
				return buildFailureFromAny(
					'When using OpenAI in a bring your own key configuration, you must provide a valid token'
				);
			}

			return ok(new OpenAIClient(openAIKey, openAIModelName));
		}

		if (modelKind === ModelKind.Anthropic) {
			const anthropicModelName = await this.getAnthropicModelName();
			const anthropicKey = await this.getAnthropicKey();

			if (!anthropicKey) {
				return buildFailureFromAny(
					'When using Anthropic in a bring your own key configuration, you must provide a valid token'
				);
			}

			return ok(new AnthropicAIClient(anthropicKey, anthropicModelName));
		}

		return buildFailureFromAny('Failed to build ai client');
	}

	async summarizeCommit({
		diffInput,
		useEmojiStyle = false,
		useBriefStyle = false,
		commitTemplate,
		onToken,
		branchName
	}: SummarizeCommitOpts): Promise<Result<string, Error>> {
		const aiClientResult = await this.buildClient();
		if (isFailure(aiClientResult)) return aiClientResult;
		const aiClient = aiClientResult.value;

		const diffLengthLimit = await this.getDiffLengthLimitConsideringAPI();
		const defaultedCommitTemplate = commitTemplate || aiClient.defaultCommitTemplate;

		const prompt = defaultedCommitTemplate.map((promptMessage) => {
			if (promptMessage.role !== MessageRole.User) {
				return promptMessage;
			}

			let content = promptMessage.content.replaceAll(
				'%{diff}',
				buildDiff(diffInput, diffLengthLimit)
			);

			const briefPart = useBriefStyle
				? 'The commit message must be only one sentence and as short as possible.'
				: '';
			content = content.replaceAll('%{brief_style}', briefPart);

			const emojiPart = useEmojiStyle
				? 'Make use of GitMoji in the title prefix.'
				: "Don't use any emoji.";
			content = content.replaceAll('%{emoji_style}', emojiPart);

			if (branchName) {
				content = content.replaceAll('%{branch_name}', branchName);
			}

			return {
				role: MessageRole.User,
				content
			};
		});

		const messageResult = await aiClient.evaluate(prompt, { onToken });
		if (isFailure(messageResult)) return messageResult;
		let message = messageResult.value;

		if (useBriefStyle) {
			message = message.split('\n')[0] ?? message;
		}

		const { title, description } = splitMessage(message);
		return ok(description ? `${title}\n\n${description}` : title);
	}

	async summarizeBranch({
		hunks,
		branchTemplate,
		onToken
	}: SummarizeBranchOpts): Promise<Result<string, Error>> {
		const aiClientResult = await this.buildClient();
		if (isFailure(aiClientResult)) return aiClientResult;
		const aiClient = aiClientResult.value;

		const diffLengthLimit = await this.getDiffLengthLimitConsideringAPI();
		const defaultedBranchTemplate = branchTemplate || aiClient.defaultBranchTemplate;
		const prompt = defaultedBranchTemplate.map((promptMessage) => {
			if (promptMessage.role !== MessageRole.User) {
				return promptMessage;
			}

			return {
				role: MessageRole.User,
				content: promptMessage.content.replaceAll('%{diff}', buildDiff(hunks, diffLengthLimit))
			};
		});

		const messageResult = await aiClient.evaluate(prompt, { onToken });
		if (isFailure(messageResult)) return messageResult;

		const message = messageResult.value;

		return ok(message?.replaceAll(' ', '-').replaceAll('\n', '-') ?? '');
	}

	async describePR({
		commitMessages,
		title,
		body,
		directive,
		prTemplate,
		prBodyTemplate,
		onToken
	}: SummarizePROpts): Promise<Result<string, Error>> {
		const aiClientResult = await this.buildClient();
		if (isFailure(aiClientResult)) return aiClientResult;
		const aiClient = aiClientResult.value;
		const defaultPRTemplate = prTemplate ?? aiClient.defaultPRTemplate;

		const mainDirective = (directive ?? this.prSummaryMainDirective) + '\n';
		const prBodyTemplateDirective = getPrTemplateDirective(prBodyTemplate);

		const prompt: Prompt = defaultPRTemplate.map((message) => {
			if (message.role !== MessageRole.User) {
				return message;
			}

			return {
				role: MessageRole.User,
				content: message.content
					.replaceAll('%{pr_main_directive}', mainDirective)
					.replaceAll('%{pr_template_directive}', prBodyTemplateDirective)
					.replaceAll('%{commit_messages}', commitMessages.slice().reverse().join('\n<###>\n'))
					.replaceAll('%{title}', title)
					.replaceAll('%{body}', body)
			};
		});

		const messageResult = await aiClient.evaluate(prompt, {
			onToken,
			maxTokens: prDescriptionTokenLimit
		});
		if (isFailure(messageResult)) return messageResult;

		let message = messageResult.value.trim();
		if (message.startsWith('```\n') && message.endsWith('\n```')) {
			message = message.slice(4, -4);
		}

		return ok(message);
	}
}
