import { useQueryClient, useSuspenseQueries } from "@tanstack/react-query";
import { useState, type FC } from "react";
import type { AiConfiguration, AiConfigurationUpdate } from "@gitbutler/but-sdk";
import { aiConfigurationQueryOptions, userProfileQueryOptions } from "#ui/api/queries.ts";
import { Button } from "@gitbutler/ui-react/Button.tsx";
import { classes } from "@gitbutler/ui-react/classes.ts";
import { FieldControlStyles } from "@gitbutler/ui-react/Field.tsx";
import { Icon } from "@gitbutler/ui-react/Icon.tsx";
import { RelativeTime } from "@gitbutler/ui-react/RelativeTime.tsx";
import { Select } from "@gitbutler/ui-react/Select.tsx";
import { Tooltip } from "@gitbutler/ui-react/Tooltip.tsx";
import { errorMessageForToast } from "#ui/errors.ts";
import {
	anthropicModels,
	configurationUpdate,
	modelSelection,
	openAiModels,
	saveThenTest,
} from "./ai-settings.ts";
import { Note, Row, Section } from "./Section.tsx";
import styles from "./Ai.module.css";

type Provider = AiConfigurationUpdate["provider"];
type KeyOption = AiConfiguration["openaiKeyOption"];

const providers: ReadonlyArray<{ value: Provider; label: string }> = [
	{ value: "openai", label: "OpenAI" },
	{ value: "anthropic", label: "Anthropic" },
	{ value: "ollama", label: "Ollama" },
	{ value: "lmstudio", label: "LM Studio" },
];

const keyOptions: ReadonlyArray<{ value: KeyOption; label: string }> = [
	{ value: "butlerAPI", label: "GitButler" },
	{ value: "bringYourOwn", label: "Your own key" },
];

const modelLabels: Record<string, string> = {
	"gpt-5.4": "GPT 5.4",
	"gpt-5.4-mini": "GPT 5.4 Mini",
	"gpt-5.4-nano": "GPT 5.4 Nano",
	"claude-haiku-4-5": "Haiku",
	"claude-sonnet-4-6": "Sonnet",
	"claude-opus-4-6": "Opus",
};

/** A text field at the end of a row that keeps what is typed and saves it when focus leaves. */
const TextField: FC<{
	id: string;
	type?: "text" | "url" | "password";
	placeholder?: string;
	value: string;
	onChange: (value: string) => void;
	onCommit: () => void;
}> = (p) => (
	<FieldControlStyles
		id={p.id}
		type={p.type ?? "text"}
		autoComplete="off"
		className={styles.field}
		placeholder={p.placeholder}
		value={p.value}
		onChange={(event) => p.onChange(event.currentTarget.value)}
		onBlur={p.onCommit}
		onKeyDown={(event) => (event.key === "Enter" || event.key === "Escape") && p.onCommit()}
	/>
);

/**
 * Picking a preset saves at once. Picking "Custom" only opens the name field; that saves when
 * it loses focus, so a half-typed model name is never stored.
 */
const ModelField: FC<{
	id: string;
	model: string;
	presets: ReadonlyArray<string>;
	recommended: string;
	onChange: (model: string, save: boolean) => void;
}> = (p) => {
	const selection = modelSelection(p.model, p.presets);
	return (
		<>
			<Row label="Model" hint={`${modelLabels[p.recommended] ?? p.recommended} is recommended.`}>
				<Select
					aria-label="Model"
					className={styles.select}
					items={[
						...p.presets.map((model) => ({ value: model, label: modelLabels[model] ?? model })),
						{ value: "custom", label: "Custom" },
					]}
					value={selection}
					onValueChange={(value) => {
						if (value === null) return;
						if (value === "custom") p.onChange("", false);
						else p.onChange(value, true);
					}}
				/>
			</Row>
			{selection === "custom" && (
				<Row label="Model name" htmlFor={`${p.id}-name`}>
					<TextField
						id={`${p.id}-name`}
						value={p.model}
						onChange={(model) => p.onChange(model, false)}
						onCommit={() => p.onChange(p.model, true)}
					/>
				</Row>
			)}
		</>
	);
};

type Status =
	| { kind: "untested" }
	| { kind: "testing" }
	| { kind: "connected"; testedAt: number }
	| { kind: "failed"; message: string };

export const Ai: FC = () => {
	const [{ data: configuration }, { data: profile }] = useSuspenseQueries({
		queries: [aiConfigurationQueryOptions, userProfileQueryOptions],
	});
	const client = useQueryClient();
	const [update, setUpdate] = useState<AiConfigurationUpdate>(() =>
		configurationUpdate(configuration),
	);
	const [saved, setSaved] = useState<AiConfiguration>(configuration);
	const [saving, setSaving] = useState(false);
	const [resetting, setResetting] = useState(false);
	const [status, setStatus] = useState<Status>({ kind: "untested" });

	// Keeps whatever is being typed in other fields. The keys are stored now, so
	// stop resending them; a blank key field means "keep the stored one".
	const acceptSaved = (next: AiConfiguration) => {
		client.setQueryData(aiConfigurationQueryOptions.queryKey, next);
		setSaved(next);
		setUpdate((current) => ({ ...current, openaiApiKey: undefined, anthropicApiKey: undefined }));
	};

	const persist = async (next: AiConfigurationUpdate) => {
		setSaving(true);
		try {
			acceptSaved(await window.lite.updateAiConfiguration(next));
		} catch (caught) {
			setStatus({ kind: "failed", message: errorMessageForToast(caught) });
		} finally {
			setSaving(false);
		}
	};

	// Selects save at once; text fields collect keystrokes and save on blur.
	const change = <K extends keyof AiConfigurationUpdate>(
		key: K,
		value: AiConfigurationUpdate[K],
		save: boolean,
	) => {
		const next = { ...update, [key]: value };
		setUpdate(next);
		if (save) void persist(next);
	};
	const commit = () => void persist(update);

	const test = async () => {
		setStatus({ kind: "testing" });
		try {
			await saveThenTest(update, acceptSaved, () => {});
			setStatus({ kind: "connected", testedAt: Date.now() });
		} catch (caught) {
			setStatus({ kind: "failed", message: errorMessageForToast(caught) });
		}
	};

	const reset = async () => {
		if (!window.confirm("Reset AI settings and delete all stored AI API keys?")) return;
		setResetting(true);
		try {
			const next = await window.lite.resetAiConfiguration();
			acceptSaved(next);
			setUpdate(configurationUpdate(next));
			setStatus({ kind: "untested" });
		} catch (caught) {
			setStatus({ kind: "failed", message: errorMessageForToast(caught) });
		} finally {
			setResetting(false);
		}
	};

	const provider = update.provider;
	const unsupported = saved.provider === "openrouter";
	const usesGitButler =
		(provider === "openai" && update.openaiKeyOption === "butlerAPI") ||
		(provider === "anthropic" && update.anthropicKeyOption === "butlerAPI");
	const busy = saving || resetting || status.kind === "testing";

	return (
		<>
			<Section
				footer={
					<>
						<output aria-live="polite" className={classes("text-12", "text-body", styles.status)}>
							{status.kind === "testing" ? (
								<Icon name="spinner" size={14} />
							) : (
								<span
									className={classes(
										styles.dot,
										status.kind === "connected" && styles.dotConnected,
										status.kind === "failed" && styles.dotFailed,
									)}
								/>
							)}
							{status.kind === "untested" && "Not tested yet"}
							{status.kind === "testing" && "AI is responding…"}
							{status.kind === "connected" && (
								<span>
									Connected · tested <RelativeTime timestamp={status.testedAt} />
								</span>
							)}
							{status.kind === "failed" && status.message}
						</output>
						<Button
							size="small"
							disabled={busy || (usesGitButler && profile === null)}
							onClick={() => void test()}
						>
							Test connection
						</Button>
					</>
				}
			>
				<Row label="Provider">
					<div className={styles.provider}>
						<Tooltip content="Reset AI settings">
							<Button
								iconOnly
								aria-label="Reset AI settings"
								disabled={busy || saved.isDefault}
								onClick={() => void reset()}
							>
								<Icon name="undo" />
							</Button>
						</Tooltip>
						<Select<Provider | "openrouter">
							aria-label="Provider"
							className={styles.select}
							items={
								unsupported
									? [
											{ value: "openrouter", label: "OpenRouter (not supported)", disabled: true },
											...providers,
										]
									: providers
							}
							value={unsupported ? "openrouter" : provider}
							onValueChange={(value) => {
								if (value !== null && value !== "openrouter") change("provider", value, true);
							}}
						/>
					</div>
				</Row>

				{provider === "openai" && (
					<>
						<Row label="Credentials">
							<Select
								aria-label="Credentials"
								className={styles.select}
								items={keyOptions}
								value={update.openaiKeyOption}
								onValueChange={(value) => value !== null && change("openaiKeyOption", value, true)}
							/>
						</Row>
						{update.openaiKeyOption === "bringYourOwn" && (
							<Row
								label="API key"
								htmlFor="openai-api-key"
								hint={
									saved.openaiHasApiKey ? "A key is configured. Leave blank to keep it." : undefined
								}
							>
								<TextField
									id="openai-api-key"
									type="password"
									placeholder={saved.openaiHasApiKey ? "••••••••" : "sk-…"}
									value={update.openaiApiKey ?? ""}
									onChange={(value) => change("openaiApiKey", value, false)}
									onCommit={commit}
								/>
							</Row>
						)}
						<ModelField
							id="openai-model"
							model={update.openaiModel}
							presets={openAiModels}
							recommended="gpt-5.4-nano"
							onChange={(model, save) => change("openaiModel", model, save)}
						/>
						{update.openaiKeyOption === "bringYourOwn" && (
							<Row label="Custom endpoint" htmlFor="openai-endpoint" hint="Optional.">
								<TextField
									id="openai-endpoint"
									type="url"
									placeholder="https://api.openai.com/v1"
									value={update.openaiCustomEndpoint ?? ""}
									onChange={(value) => change("openaiCustomEndpoint", value, false)}
									onCommit={commit}
								/>
							</Row>
						)}
					</>
				)}

				{provider === "anthropic" && (
					<>
						<Row label="Credentials">
							<Select
								aria-label="Credentials"
								className={styles.select}
								items={keyOptions}
								value={update.anthropicKeyOption}
								onValueChange={(value) =>
									value !== null && change("anthropicKeyOption", value, true)
								}
							/>
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
								<TextField
									id="anthropic-api-key"
									type="password"
									placeholder={saved.anthropicHasApiKey ? "••••••••" : "sk-ant-…"}
									value={update.anthropicApiKey ?? ""}
									onChange={(value) => change("anthropicApiKey", value, false)}
									onCommit={commit}
								/>
							</Row>
						)}
						<ModelField
							id="anthropic-model"
							model={update.anthropicModel}
							presets={anthropicModels}
							recommended="claude-haiku-4-5"
							onChange={(model, save) => change("anthropicModel", model, save)}
						/>
					</>
				)}

				{provider === "ollama" && (
					<>
						<Row label="Endpoint" htmlFor="ollama-endpoint" hint="Use host:port format.">
							<TextField
								id="ollama-endpoint"
								value={update.ollamaEndpoint}
								onChange={(value) => change("ollamaEndpoint", value, false)}
								onCommit={commit}
							/>
						</Row>
						<Row label="Model name" htmlFor="ollama-model">
							<TextField
								id="ollama-model"
								value={update.ollamaModel}
								onChange={(value) => change("ollamaModel", value, false)}
								onCommit={commit}
							/>
						</Row>
					</>
				)}

				{provider === "lmstudio" && (
					<>
						<Row label="Endpoint" htmlFor="lmstudio-endpoint" hint="OpenAI-compatible base URL.">
							<TextField
								id="lmstudio-endpoint"
								type="url"
								value={update.lmstudioEndpoint}
								onChange={(value) => change("lmstudioEndpoint", value, false)}
								onCommit={commit}
							/>
						</Row>
						<Row label="Model name" htmlFor="lmstudio-model">
							<TextField
								id="lmstudio-model"
								value={update.lmstudioModel}
								onChange={(value) => change("lmstudioModel", value, false)}
								onCommit={commit}
							/>
						</Row>
					</>
				)}
			</Section>

			{unsupported && (
				<Note icon="warning">
					OpenRouter is configured but is not supported in GitButler. Choose another provider, or
					reset these settings.
				</Note>
			)}

			{usesGitButler && profile === null && (
				<Note icon="warning">Sign in on the General page before using the GitButler AI API.</Note>
			)}

			<Note icon="lock">
				Configure GitButler's AI provider. API keys are safely stored in the backend.
			</Note>
		</>
	);
};
