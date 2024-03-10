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
			'gitbutler.modelKind',
			ModelKind.OpenAI
		);
		this.setModelKind = gitConfig.buildSetter<ModelKind>('gitbutler.modelKind');

		this.getKeyOption = gitConfig.buildGetterWithDefault<KeyOption>(
			'gitbutler.keyOption',
			KeyOption.ButlerAPI
		);
		this.setKeyOption = gitConfig.buildSetter<KeyOption>('gitbutler.keyOption');

		this.getOpenAIKey = gitConfig.buildGetter('gitbutler.openAIKey');
		this.setOpenAIKey = gitConfig.buildSetter('gitbutler.openAIKey');

		this.getOpenAIModel = gitConfig.buildGetterWithDefault<OpenAIModel>(
			'gitbutler.openAIModel',
			OpenAIModel.GPT35Turbo
		);
		this.setOpenAIModel = gitConfig.buildSetter<OpenAIModel>('gitbutler.keyOption');

		this.getAnthropicKey = gitConfig.buildGetter('gitbutler.AnthropicKey');
		this.setAnthropicKey = gitConfig.buildSetter('gitbutler.AnthropicKey');

		this.getAnthropicModel = gitConfig.buildGetterWithDefault<AnthropicModel>(
			'gitbutler.anthropicModel',
			AnthropicModel.Sonnet
		);
		this.setAnthropicModel = gitConfig.buildSetter<AnthropicModel>('gitbutler.anthropicModel');
	}
}
