import { derived, type Readable, type Writable } from 'svelte/store';
import type { GitConfig } from './gitConfig';

export enum ModelKind {
	OpenAI = 'openai',
	Anthropic = 'anthropic'
}

export enum KeyOption {
	BringYourOwn = 'bringYourOwn',
	ButlerAPI = 'butlerAPI'
}

export enum OpenAIModel {
	GPT35Turbo = 'gpt-3.5-turbo',
	GPT4 = 'gpt-4',
	GPT4Turbo = 'gpt-4-turbo-preview'
}

export enum AnthropicModel {
	Opus = 'claude-3-opus-20240229',
	Sonnet = 'claude-3-sonnet-20240229'
}

const modelKindConfigKey = 'gitbutler.modelKind';
const keyOptionConfigKey = 'gitbutler.keyOption';
const openAIKeyConfigKey = 'gitbutler.openAIKey';
const openAIModelConfigKey = 'gitbutler.openAIModel';
const anthropicKeyConfigKey = 'gitbutler.anthropicKey';
const anthropicModelConfigKey = 'gitbutler.anthropicModel';

export interface AllSummarizerSettings {
	modelKind: ModelKind;
	keyOption: KeyOption;
	openAIKey: string | undefined;
	openAIModel: OpenAIModel;
	anthropicKey: string | undefined;
	anthropicModel: AnthropicModel;
}

export class SummarizerSettings {
	modelKind$: Writable<ModelKind>;
	keyOption$: Writable<KeyOption>;
	openAIKey$: Writable<string | undefined>;
	openAIModel$: Writable<OpenAIModel>;
	anthropicKey$: Writable<string | undefined>;
	anthropicModel$: Writable<AnthropicModel>;
	all$: Readable<AllSummarizerSettings>;

	constructor(gitConfig: GitConfig) {
		this.modelKind$ = gitConfig.buildWritableWithDefault<ModelKind>(
			modelKindConfigKey,
			ModelKind.OpenAI
		);
		this.keyOption$ = gitConfig.buildWritableWithDefault<KeyOption>(
			keyOptionConfigKey,
			KeyOption.ButlerAPI
		);
		this.openAIKey$ = gitConfig.buildWritable(openAIKeyConfigKey);
		this.openAIModel$ = gitConfig.buildWritableWithDefault<OpenAIModel>(
			openAIModelConfigKey,
			OpenAIModel.GPT35Turbo
		);
		this.anthropicKey$ = gitConfig.buildWritable(anthropicKeyConfigKey);
		this.anthropicModel$ = gitConfig.buildWritableWithDefault<AnthropicModel>(
			anthropicModelConfigKey,
			AnthropicModel.Sonnet
		);

		this.all$ = derived(
			[
				this.modelKind$,
				this.keyOption$,
				this.openAIKey$,
				this.openAIModel$,
				this.anthropicKey$,
				this.anthropicModel$
			],
			([modelKind, keyOption, openAIKey, openAIModel, anthropicKey, anthropicModel]) => ({
				modelKind,
				keyOption,
				openAIKey,
				openAIModel,
				anthropicKey,
				anthropicModel
			})
		);
	}
}
