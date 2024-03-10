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

export class SummarizerSettings {
	getModelKind: () => Promise<ModelKind>;
	setModelKind: (value: ModelKind) => Promise<ModelKind | null>;
	getKeyOption: () => Promise<KeyOption>;
	setKeyOption: (value: KeyOption) => Promise<KeyOption | null>;
	getOpenAIKey: () => Promise<string | null>;
	setOpenAIKey: (value: string) => Promise<string | null>;
	getOpenAIModel: () => Promise<OpenAIModel>;
	setOpenAIModel: (value: OpenAIModel) => Promise<OpenAIModel | null>;
	getAnthropicKey: () => Promise<string | null>;
	setAnthropicKey: (value: string) => Promise<string | null>;
	getAnthropicModel: () => Promise<AnthropicModel>;
	setAnthropicModel: (value: AnthropicModel) => Promise<AnthropicModel | null>;

	constructor(gitConfig: GitConfig) {
		this.getModelKind = gitConfig.buildGetterWithDefault<ModelKind>(
			modelKindConfigKey,
			ModelKind.OpenAI
		);
		this.setModelKind = gitConfig.buildSetter<ModelKind>(modelKindConfigKey);

		this.getKeyOption = gitConfig.buildGetterWithDefault<KeyOption>(
			keyOptionConfigKey,
			KeyOption.ButlerAPI
		);
		this.setKeyOption = gitConfig.buildSetter<KeyOption>(keyOptionConfigKey);

		this.getOpenAIKey = gitConfig.buildGetter(openAIKeyConfigKey);
		this.setOpenAIKey = gitConfig.buildSetter(openAIKeyConfigKey);

		this.getOpenAIModel = gitConfig.buildGetterWithDefault<OpenAIModel>(
			openAIModelConfigKey,
			OpenAIModel.GPT35Turbo
		);
		this.setOpenAIModel = gitConfig.buildSetter<OpenAIModel>(keyOptionConfigKey);

		this.getAnthropicKey = gitConfig.buildGetter(anthropicKeyConfigKey);
		this.setAnthropicKey = gitConfig.buildSetter(anthropicKeyConfigKey);

		this.getAnthropicModel = gitConfig.buildGetterWithDefault<AnthropicModel>(
			anthropicModelConfigKey,
			AnthropicModel.Sonnet
		);
		this.setAnthropicModel = gitConfig.buildSetter<AnthropicModel>(anthropicModelConfigKey);
	}
}
