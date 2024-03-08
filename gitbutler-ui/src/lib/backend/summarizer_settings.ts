import { ModelKind, KeyOption } from '$lib/backend/aiProviders';
import { invoke } from '@tauri-apps/api/tauri';

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

export async function getTokenOption(): Promise<KeyOption> {
	const tokenKind = (await gitGetConfig(tokenOptionConfigKey)) as KeyOption | undefined;
	return tokenKind || KeyOption.ButlerAPI;
}

export function setTokenOption(tokenOption: KeyOption) {
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

const anthropicKeyConfigKey = 'anthropicKey';

export async function getAnthropicKey(): Promise<string | undefined> {
	const key = await gitGetConfig(anthropicKeyConfigKey);
	return key || undefined;
}

export function setAnthropicToken(token: string) {
	return gitSetConfig(anthropicKeyConfigKey, token);
}
