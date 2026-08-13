import { useQueryClient, useSuspenseQueries } from "@tanstack/react-query";
import { useState, type FC } from "react";
import type { AiConfiguration, AiConfigurationUpdate } from "@gitbutler/but-sdk";
import { aiConfigurationQueryOptions, userProfileQueryOptions } from "#ui/api/queries.ts";
import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	anthropicModels,
	configurationUpdate,
	modelSelection,
	openAiModels,
	saveThenTest,
} from "./ai-settings.ts";
import { Row, Section } from "./Section.tsx";
import styles from "./Ai.module.css";

type Provider = AiConfigurationUpdate["provider"];
type KeyOption = AiConfiguration["openaiKeyOption"];

const providerLabels: Record<Provider, string> = {
	openai: "OpenAI",
	anthropic: "Anthropic",
	ollama: "Ollama",
	lmstudio: "LM Studio",
};

const modelLabels: Record<string, string> = {
	"gpt-5.4": "GPT 5.4",
	"gpt-5.4-mini": "GPT 5.4 Mini",
	"gpt-5.4-nano": "GPT 5.4 Nano (recommended)",
	"claude-haiku-4-5": "Haiku (recommended)",
	"claude-sonnet-4-6": "Sonnet",
	"claude-opus-4-6": "Opus",
};

const ModelField: FC<{
	id: string;
	label: string;
	model: string;
	presets: ReadonlyArray<string>;
	onChange: (model: string) => void;
}> = (p) => {
	const selection = modelSelection(p.model, p.presets);
	return (
		<>
			<Row label={p.label} htmlFor={`${p.id}-preset`}>
				<select
					id={`${p.id}-preset`}
					value={selection}
					onChange={(event) => {
						const value = event.currentTarget.value;
						p.onChange(value === "custom" ? "" : value);
					}}
				>
					{p.presets.map((model) => (
						<option key={model} value={model}>
							{modelLabels[model] ?? model}
						</option>
					))}
					<option value="custom">Custom…</option>
				</select>
			</Row>
			{selection === "custom" && (
				<Row label="Custom model" htmlFor={`${p.id}-custom`}>
					<input
						id={`${p.id}-custom`}
						type="text"
						value={p.model}
						onChange={(event) => p.onChange(event.currentTarget.value)}
					/>
				</Row>
			)}
		</>
	);
};

export const Ai: FC = () => {
	const [{ data: configuration }, { data: profile }] = useSuspenseQueries({
		queries: [aiConfigurationQueryOptions, userProfileQueryOptions],
	});
	const client = useQueryClient();
	const [update, setUpdate] = useState<AiConfigurationUpdate>(() =>
		configurationUpdate(configuration),
	);
	const [saved, setSaved] = useState<AiConfiguration>(configuration);
	const [dirty, setDirty] = useState(configuration.provider === "openrouter");
	const [saving, setSaving] = useState(false);
	const [resetting, setResetting] = useState(false);
	const [testing, setTesting] = useState(false);
	const [result, setResult] = useState("");
	const [error, setError] = useState<string | null>(null);

	const change = <K extends keyof AiConfigurationUpdate>(
		key: K,
		value: AiConfigurationUpdate[K],
	) => {
		setDirty(true);
		setUpdate((current) => ({ ...current, [key]: value }));
	};

	const acceptSaved = (next: AiConfiguration) => {
		client.setQueryData(aiConfigurationQueryOptions.queryKey, next);
		setSaved(next);
		setUpdate(configurationUpdate(next));
		setDirty(false);
	};

	const save = async () => {
		setSaving(true);
		setError(null);
		try {
			acceptSaved(await window.lite.updateAiConfiguration(update));
		} catch (caught) {
			setError(errorMessageForToast(caught));
		} finally {
			setSaving(false);
		}
	};

	const test = async () => {
		setTesting(true);
		setError(null);
		setResult("");
		try {
			const response = await saveThenTest(update, acceptSaved, (token) =>
				setResult((current) => current + token),
			);
			setResult(response);
		} catch (caught) {
			setError(errorMessageForToast(caught));
		} finally {
			setTesting(false);
		}
	};

	const reset = async () => {
		if (!window.confirm("Reset AI settings and delete all stored AI API keys?")) return;
		setResetting(true);
		setError(null);
		setResult("");
		try {
			acceptSaved(await window.lite.resetAiConfiguration());
		} catch (caught) {
			setError(errorMessageForToast(caught));
		} finally {
			setResetting(false);
		}
	};

	const provider = update.provider;
	const usesGitButler =
		(provider === "openai" && update.openaiKeyOption === "butlerAPI") ||
		(provider === "anthropic" && update.anthropicKeyOption === "butlerAPI");
	const busy = saving || testing || resetting;

	return (
		<>
			<p className={classes("text-13", styles.intro)}>
				Configure the Rust AI provider used by GitButler Lite. API keys stay in secure backend
				storage.
			</p>

			<Section>
				<Row label="Provider" htmlFor="ai-provider">
					<select
						id="ai-provider"
						value={provider}
						onChange={(event) => change("provider", event.currentTarget.value as Provider)}
					>
						{Object.entries(providerLabels).map(([value, label]) => (
							<option key={value} value={value}>
								{label}
							</option>
						))}
					</select>
				</Row>

				{provider === "openai" && (
					<>
						<Row label="Credentials" htmlFor="openai-credentials">
							<select
								id="openai-credentials"
								value={update.openaiKeyOption}
								onChange={(event) =>
									change("openaiKeyOption", event.currentTarget.value as KeyOption)
								}
							>
								<option value="butlerAPI">GitButler account</option>
								<option value="bringYourOwn">Your own key</option>
							</select>
						</Row>
						{update.openaiKeyOption === "bringYourOwn" && (
							<Row
								label="API key"
								htmlFor="openai-api-key"
								hint={
									saved.openaiHasApiKey ? "A key is configured. Leave blank to keep it." : undefined
								}
							>
								<input
									id="openai-api-key"
									type="password"
									autoComplete="off"
									placeholder={saved.openaiHasApiKey ? "••••••••" : "sk-…"}
									value={update.openaiApiKey ?? ""}
									onChange={(event) => change("openaiApiKey", event.currentTarget.value)}
								/>
							</Row>
						)}
						<ModelField
							id="openai-model"
							label="Model"
							model={update.openaiModel}
							presets={openAiModels}
							onChange={(model) => change("openaiModel", model)}
						/>
						{update.openaiKeyOption === "bringYourOwn" && (
							<Row label="Custom endpoint" htmlFor="openai-endpoint" hint="Optional.">
								<input
									id="openai-endpoint"
									type="url"
									placeholder="https://api.openai.com/v1"
									value={update.openaiCustomEndpoint ?? ""}
									onChange={(event) => change("openaiCustomEndpoint", event.currentTarget.value)}
								/>
							</Row>
						)}
					</>
				)}

				{provider === "anthropic" && (
					<>
						<Row label="Credentials" htmlFor="anthropic-credentials">
							<select
								id="anthropic-credentials"
								value={update.anthropicKeyOption}
								onChange={(event) =>
									change("anthropicKeyOption", event.currentTarget.value as KeyOption)
								}
							>
								<option value="butlerAPI">GitButler account</option>
								<option value="bringYourOwn">Your own key</option>
							</select>
						</Row>
						{update.anthropicKeyOption === "bringYourOwn" && (
							<Row
								label="API key"
								htmlFor="anthropic-api-key"
								hint={
									saved.anthropicHasApiKey
										? "A key is configured. Leave blank to keep it."
										: undefined
								}
							>
								<input
									id="anthropic-api-key"
									type="password"
									autoComplete="off"
									placeholder={saved.anthropicHasApiKey ? "••••••••" : "sk-ant-…"}
									value={update.anthropicApiKey ?? ""}
									onChange={(event) => change("anthropicApiKey", event.currentTarget.value)}
								/>
							</Row>
						)}
						<ModelField
							id="anthropic-model"
							label="Model"
							model={update.anthropicModel}
							presets={anthropicModels}
							onChange={(model) => change("anthropicModel", model)}
						/>
					</>
				)}

				{provider === "ollama" && (
					<>
						<Row label="Endpoint" htmlFor="ollama-endpoint" hint="Use host:port format.">
							<input
								id="ollama-endpoint"
								type="text"
								value={update.ollamaEndpoint}
								onChange={(event) => change("ollamaEndpoint", event.currentTarget.value)}
							/>
						</Row>
						<Row label="Model" htmlFor="ollama-model">
							<input
								id="ollama-model"
								type="text"
								value={update.ollamaModel}
								onChange={(event) => change("ollamaModel", event.currentTarget.value)}
							/>
						</Row>
					</>
				)}

				{provider === "lmstudio" && (
					<>
						<Row label="Endpoint" htmlFor="lmstudio-endpoint" hint="OpenAI-compatible base URL.">
							<input
								id="lmstudio-endpoint"
								type="url"
								value={update.lmstudioEndpoint}
								onChange={(event) => change("lmstudioEndpoint", event.currentTarget.value)}
							/>
						</Row>
						<Row label="Model" htmlFor="lmstudio-model">
							<input
								id="lmstudio-model"
								type="text"
								value={update.lmstudioModel}
								onChange={(event) => change("lmstudioModel", event.currentTarget.value)}
							/>
						</Row>
					</>
				)}
			</Section>

			{saved.provider === "openrouter" && (
				<p className={classes("text-12", styles.warning)}>
					OpenRouter is configured but is not supported in Lite. Save to switch providers, or reset
					these settings.
				</p>
			)}

			{usesGitButler && profile === null && (
				<p className={classes("text-12", styles.warning)}>
					Sign in on the General page before using the GitButler AI API.
				</p>
			)}

			<div className={styles.actions}>
				<button
					type="button"
					className={classes(getButtonClassName({ size: "small" }), styles.reset)}
					disabled={busy}
					onClick={() => void reset()}
				>
					{resetting ? "Resetting…" : "Reset settings"}
				</button>
				<button
					type="button"
					className={getButtonClassName({ size: "small" })}
					disabled={busy || !dirty}
					onClick={() => void save()}
				>
					{saving ? "Saving…" : "Save"}
				</button>
				<button
					type="button"
					className={getButtonClassName({ variant: "pop", size: "small" })}
					disabled={busy || (usesGitButler && profile === null)}
					onClick={() => void test()}
				>
					{testing ? "AI is responding…" : "Test connection"}
				</button>
			</div>

			{(result !== "" || error !== null) && (
				<output
					aria-live="polite"
					className={classes("text-12", styles.result, error !== null && styles.resultError)}
				>
					{error ?? result}
				</output>
			)}
		</>
	);
};
