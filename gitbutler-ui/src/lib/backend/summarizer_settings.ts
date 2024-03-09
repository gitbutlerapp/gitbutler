import { invoke } from '@tauri-apps/api/tauri';

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

function gitGetConfig(key: string) {
	return invoke<string>('git_get_global_config', { key: `gitbutler.${key}` });
}

function gitSetConfig(key: string, value: string) {
	return invoke<string>('git_set_global_config', { key: `gitbutler.${key}`, value });
}

const modelKindConfigKey = 'modelKind';

export async function getModelKind(): Promise<ModelKind> {
	const modelKind = (await gitGetConfig(modelKindConfigKey)) as ModelKind | undefined;
	return modelKind || ModelKind.OpenAI;
}

export function setModelKind(modelKind: ModelKind) {
	return gitSetConfig(modelKindConfigKey, modelKind);
}

const tokenOptionConfigKey = 'tokenOption';

export async function getKeyOption(): Promise<KeyOption> {
	const tokenKind = (await gitGetConfig(tokenOptionConfigKey)) as KeyOption | undefined;
	return tokenKind || KeyOption.ButlerAPI;
}

export function setKeyOption(tokenOption: KeyOption) {
	return gitSetConfig(tokenOptionConfigKey, tokenOption);
}

const openAIKeyConfigKey = 'openAIKey';

export async function getOpenAIKey(): Promise<string | undefined> {
	const key = await gitGetConfig(openAIKeyConfigKey);
	return key || undefined;
}

export function setOpenAIKey(token: string) {
	return gitSetConfig(openAIKeyConfigKey, token);
}

const openAIModelConfigKey = 'openAIModel';

export async function getOpenAIModel(): Promise<OpenAIModel> {
	const model = (await gitGetConfig(openAIModelConfigKey)) as OpenAIModel | undefined;
	return model || OpenAIModel.GPT35Turbo;
}

export async function setOpenAIModel(model: OpenAIModel) {
	return gitSetConfig(openAIModelConfigKey, model);
}

const anthropicKeyConfigKey = 'anthropicKey';

export async function getAnthropicKey(): Promise<string | undefined> {
	const key = await gitGetConfig(anthropicKeyConfigKey);
	return key || undefined;
}

export function setAnthropicKey(token: string) {
	return gitSetConfig(anthropicKeyConfigKey, token);
}

const anthropicModelConfigKey = 'anthropicModel';

export async function getAnthropicModel(): Promise<AnthropicModel> {
	const model = (await gitGetConfig(anthropicModelConfigKey)) as AnthropicModel | undefined;
	return model || AnthropicModel.Sonnet;
}

export async function setAnthropicModel(model: AnthropicModel) {
	return gitSetConfig(anthropicModelConfigKey, model);
}
