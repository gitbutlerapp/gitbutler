import type { AiConfiguration, AiConfigurationUpdate } from "@gitbutler/but-sdk";

export const openAiModels = ["gpt-5.4", "gpt-5.4-mini", "gpt-5.4-nano"] as const;
export const anthropicModels = [
	"claude-haiku-4-5",
	"claude-sonnet-4-6",
	"claude-opus-4-6",
] as const;

export const modelSelection = (model: string, presets: ReadonlyArray<string>) =>
	presets.includes(model) ? model : "custom";

export const configurationUpdate = (configuration: AiConfiguration): AiConfigurationUpdate => ({
	provider: configuration.provider === "openrouter" ? "openai" : configuration.provider,
	openaiKeyOption: configuration.openaiKeyOption,
	openaiModel: configuration.openaiModel,
	openaiCustomEndpoint: configuration.openaiCustomEndpoint,
	openaiApiKey: undefined,
	anthropicKeyOption: configuration.anthropicKeyOption,
	anthropicModel: configuration.anthropicModel,
	anthropicApiKey: undefined,
	ollamaEndpoint: configuration.ollamaEndpoint,
	ollamaModel: configuration.ollamaModel,
	lmstudioEndpoint: configuration.lmstudioEndpoint,
	lmstudioModel: configuration.lmstudioModel,
});

export const saveThenTest = async (
	update: AiConfigurationUpdate,
	onSaved: (configuration: AiConfiguration) => void,
	onToken: (token: string) => void,
) => {
	const configuration = await window.lite.updateAiConfiguration(update);
	onSaved(configuration);
	return window.lite.streamAiResponse(
		"You are checking whether an AI provider is configured correctly.",
		"Reply with one short sentence confirming the connection works.",
		onToken,
	);
};
