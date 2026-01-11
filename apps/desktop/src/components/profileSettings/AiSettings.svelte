<script lang="ts">
	import AIPromptEdit from '$components/AIPromptEdit.svelte';
	import AiCredentialCheck from '$components/AiCredentialCheck.svelte';
	import AuthorizationBanner from '$components/AuthorizationBanner.svelte';
	import SettingsSection from '$components/SettingsSection.svelte';
	import { AISecretHandle, AI_SERVICE, GitAIConfigKey, KeyOption } from '$lib/ai/service';
	import { OpenAIModelName, AnthropicModelName, ModelKind } from '$lib/ai/types';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { SECRET_SERVICE } from '$lib/secrets/secretsService';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import {
		CardGroup,
		Icon,
		InfoMessage,
		RadioButton,
		Select,
		SelectItem,
		Spacer,
		Textbox
	} from '@gitbutler/ui';

	import { onMount, tick } from 'svelte';
	import { run } from 'svelte/legacy';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const gitConfigService = inject(GIT_CONFIG_SERVICE);
	const secretsService = inject(SECRET_SERVICE);
	const aiService = inject(AI_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = userService.user;
	let initialized = false;

	let modelKind: ModelKind | undefined = $state();
	let openAIKeyOption: KeyOption | undefined = $state();
	let anthropicKeyOption: KeyOption | undefined = $state();
	let openAIKey: string | undefined = $state();
	let openAICustomEndpoint: string | undefined = $state();
	let openAIModelName: OpenAIModelName | undefined = $state();
	let anthropicKey: string | undefined = $state();
	let anthropicModelName: AnthropicModelName | undefined = $state();
	let diffLengthLimit: number | undefined = $state();
	let ollamaEndpoint: string | undefined = $state();
	let ollamaModel: string | undefined = $state();
	let lmStudioEndpoint: string | undefined = $state();
	let lmStudioModel: string | undefined = $state();

	async function setConfiguration(key: GitAIConfigKey, value: string | undefined) {
		if (!initialized) return;
		gitConfigService.set(key, value || '');
	}

	async function setSecret(handle: AISecretHandle, secret: string | undefined) {
		if (!initialized) return;
		await secretsService.set(handle, secret || '');
	}

	onMount(async () => {
		modelKind = await aiService.getModelKind();

		openAIKeyOption = await aiService.getOpenAIKeyOption();
		openAIModelName = await aiService.getOpenAIModleName();
		openAIKey = await aiService.getOpenAIKey();
		openAICustomEndpoint = await aiService.getOpenAICustomEndpoint();

		anthropicKeyOption = await aiService.getAnthropicKeyOption();
		anthropicModelName = await aiService.getAnthropicModelName();
		anthropicKey = await aiService.getAnthropicKey();

		diffLengthLimit = await aiService.getDiffLengthLimit();

		ollamaEndpoint = await aiService.getOllamaEndpoint();
		ollamaModel = await aiService.getOllamaModelName();

		lmStudioEndpoint = await aiService.getLMStudioEndpoint();
		lmStudioModel = await aiService.getLMStudioModelName();

		// Ensure reactive declarations have finished running before we set initialized to true
		await tick();

		initialized = true;
	});

	const keyOptions = $derived([
		{
			label: $t('settings.general.ai.useButlerApi'),
			value: KeyOption.ButlerAPI
		},
		{
			label: $t('settings.general.ai.bringYourOwn'),
			value: KeyOption.BringYourOwn
		}
	]);

	const openAIModelOptions = $derived([
		{
			label: $t('settings.general.ai.modelNames.gpt5'),
			value: OpenAIModelName.GPT5
		},
		{
			label: $t('settings.general.ai.modelNames.gpt5Mini'),
			value: OpenAIModelName.GPT5Mini
		},
		{
			label: $t('settings.general.ai.modelNames.o3Mini'),
			value: OpenAIModelName.O3mini
		},
		{
			label: $t('settings.general.ai.modelNames.o1Mini'),
			value: OpenAIModelName.O1mini
		},
		{
			label: $t('settings.general.ai.modelNames.gpt4oMini'),
			value: OpenAIModelName.GPT4oMini
		},
		{
			label: $t('settings.general.ai.modelNames.gpt41'),
			value: OpenAIModelName.GPT4_1
		},
		{
			label: $t('settings.general.ai.modelNames.gpt41Mini'),
			value: OpenAIModelName.GPT4_1Mini
		}
	]);

	const anthropicModelOptions = $derived([
		{
			label: $t('settings.general.ai.modelNames.haiku'),
			value: AnthropicModelName.Haiku
		},
		{
			label: $t('settings.general.ai.modelNames.sonnet35'),
			value: AnthropicModelName.Sonnet35
		},
		{
			label: $t('settings.general.ai.modelNames.sonnet37'),
			value: AnthropicModelName.Sonnet37
		},
		{
			label: $t('settings.general.ai.modelNames.sonnet4'),
			value: AnthropicModelName.Sonnet4
		},
		{
			label: $t('settings.general.ai.modelNames.opus4'),
			value: AnthropicModelName.Opus4
		}
	]);

	let form = $state<HTMLFormElement>();

	function onFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		modelKind = formData.get('modelKind') as ModelKind;
	}
	run(() => {
		setConfiguration(GitAIConfigKey.ModelProvider, modelKind);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.OpenAIKeyOption, openAIKeyOption);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.OpenAIModelName, openAIModelName);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.OpenAICustomEndpoint, openAICustomEndpoint);
	});
	run(() => {
		setSecret(AISecretHandle.OpenAIKey, openAIKey);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.AnthropicKeyOption, anthropicKeyOption);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.AnthropicModelName, anthropicModelName);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.DiffLengthLimit, diffLengthLimit?.toString());
	});
	run(() => {
		setSecret(AISecretHandle.AnthropicKey, anthropicKey);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.OllamaEndpoint, ollamaEndpoint);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.OllamaModelName, ollamaModel);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.LMStudioEndpoint, lmStudioEndpoint);
	});
	run(() => {
		setConfiguration(GitAIConfigKey.LMStudioModelName, lmStudioModel);
	});
	run(() => {
		if (form) form.modelKind.value = modelKind;
	});
</script>

{#snippet shortNote(text: string)}
	<div class="ai-settings__short-note">
		<Icon name="info-small" />
		<p class="text-12 text-body">{text}</p>
	</div>
{/snippet}

<p class="text-13 text-body ai-settings__about-text">
	{$t('settings.general.ai.about')}
</p>

<CardGroup>
	<form class="git-radio" bind:this={form} onchange={(e) => onFormChange(e.currentTarget)}>
		<CardGroup.Item labelFor="open-ai">
			{#snippet title()}
				{$t('settings.general.ai.openAi.title')}
			{/snippet}
			{#snippet actions()}
				<RadioButton name="modelKind" id="open-ai" value={ModelKind.OpenAI} />
			{/snippet}
		</CardGroup.Item>
		{#if modelKind === ModelKind.OpenAI}
			<CardGroup.Item>
				<Select
					value={openAIKeyOption}
					options={keyOptions}
					wide
					label={$t('settings.general.ai.openAi.keyPrompt')}
					onselect={(value) => {
						openAIKeyOption = value as KeyOption;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === openAIKeyOption} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				{#if openAIKeyOption === KeyOption.ButlerAPI}
					{#if !$user}
						<AuthorizationBanner message={$t('settings.general.ai.openAi.signInMessage')} />
					{:else}
						{@render shortNote($t('settings.general.ai.openAi.butlerApiNote'))}
					{/if}
				{/if}

				{#if openAIKeyOption === KeyOption.BringYourOwn}
					<Textbox
						label={$t('settings.general.ai.openAi.keyLabel')}
						type="password"
						bind:value={openAIKey}
						required
						placeholder="sk-..."
					/>

					<Select
						value={openAIModelName}
						options={openAIModelOptions}
						label={$t('settings.general.ai.openAi.modelVersion')}
						wide
						onselect={(value) => {
							openAIModelName = value as OpenAIModelName;
						}}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === openAIModelName} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>

					<Textbox
						label={$t('settings.general.ai.openAi.customEndpoint')}
						bind:value={openAICustomEndpoint}
						placeholder="https://api.openai.com/v1"
					/>
				{/if}
			</CardGroup.Item>
		{/if}

		<CardGroup.Item labelFor="anthropic">
			{#snippet title()}
				{$t('settings.general.ai.anthropic.title')}
			{/snippet}
			{#snippet actions()}
				<RadioButton name="modelKind" id="anthropic" value={ModelKind.Anthropic} />
			{/snippet}
		</CardGroup.Item>
		{#if modelKind === ModelKind.Anthropic}
			<CardGroup.Item>
				<Select
					value={anthropicKeyOption}
					options={keyOptions}
					wide
					label={$t('settings.general.ai.anthropic.keyPrompt')}
					onselect={(value) => {
						anthropicKeyOption = value as KeyOption;
					}}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem selected={item.value === anthropicKeyOption} {highlighted}>
							{item.label}
						</SelectItem>
					{/snippet}
				</Select>

				{#if anthropicKeyOption === KeyOption.ButlerAPI}
					{#if !$user}
						<AuthorizationBanner message={$t('settings.general.ai.anthropic.signInMessage')} />
					{:else}
						{@render shortNote($t('settings.general.ai.anthropic.butlerApiNote'))}
					{/if}
				{/if}

				{#if anthropicKeyOption === KeyOption.BringYourOwn}
					<Textbox
						label={$t('settings.general.ai.anthropic.keyLabel')}
						type="password"
						bind:value={anthropicKey}
						required
						placeholder="sk-ant-api03-..."
					/>

					<Select
						value={anthropicModelName}
						options={anthropicModelOptions}
						label={$t('settings.general.ai.anthropic.modelVersion')}
						onselect={(value) => {
							anthropicModelName = value as AnthropicModelName;
						}}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === anthropicModelName} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				{/if}
			</CardGroup.Item>
		{/if}

		<CardGroup.Item labelFor="ollama">
			{#snippet title()}
				{$t('settings.general.ai.ollama.title')}
			{/snippet}
			{#snippet actions()}
				<RadioButton name="modelKind" id="ollama" value={ModelKind.Ollama} />
			{/snippet}
		</CardGroup.Item>
		{#if modelKind === ModelKind.Ollama}
			<CardGroup.Item>
				<Textbox
					label="Endpoint"
					bind:value={ollamaEndpoint}
					placeholder="http://127.0.0.1:11434"
				/>
				<Textbox label="Model" bind:value={ollamaModel} placeholder="llama3" />
				<InfoMessage filled outlined={false}>
					{#snippet title()}
						{$t('settings.general.ai.ollama.configTitle')}
					{/snippet}
					{#snippet content()}
						{@html $t('settings.general.ai.ollama.configContent')}
					{/snippet}
				</InfoMessage>
			</CardGroup.Item>
		{/if}

		<CardGroup.Item labelFor="lmstudio">
			{#snippet title()}
				{$t('settings.general.ai.lmStudio.title')}
			{/snippet}
			{#snippet actions()}
				<RadioButton name="modelKind" id="lmstudio" value={ModelKind.LMStudio} />
			{/snippet}
		</CardGroup.Item>
		{#if modelKind === ModelKind.LMStudio}
			<CardGroup.Item>
				<Textbox
					label={$t('settings.general.ai.lmStudio.endpoint')}
					bind:value={lmStudioEndpoint}
					placeholder="http://127.0.0.1:1234"
				/>
				<Textbox
					label={$t('settings.general.ai.lmStudio.model')}
					bind:value={lmStudioModel}
					placeholder="default"
				/>
				<InfoMessage filled outlined={false}>
					{#snippet title()}
						{$t('settings.general.ai.lmStudio.configTitle')}
					{/snippet}
					{#snippet content()}
						<div class="ai-settings__section-text-block">
							{@html $t('settings.general.ai.lmStudio.configContent')}
						</div>
					{/snippet}
				</InfoMessage>
			</CardGroup.Item>
		{/if}

		<CardGroup.Item>
			<AiCredentialCheck />
		</CardGroup.Item>
	</form>
</CardGroup>

<Spacer />

<CardGroup.Item standalone>
	{#snippet title()}
		{$t('settings.general.ai.contextLength.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.general.ai.contextLength.caption')}
	{/snippet}
	{#snippet actions()}
		<Textbox
			type="number"
			width={80}
			textAlign="center"
			value={diffLengthLimit?.toString()}
			minVal={100}
			oninput={(value: string) => {
				diffLengthLimit = parseInt(value);
			}}
			placeholder="5000"
		/>
	{/snippet}
</CardGroup.Item>

<Spacer />

<SettingsSection>
	{#snippet title()}
		{$t('settings.general.ai.customPrompts.title')}
	{/snippet}
	{#snippet description()}
		{$t('settings.general.ai.customPrompts.description')}
	{/snippet}

	<div class="prompt-groups">
		<AIPromptEdit promptUse="commits" />
		<Spacer margin={12} />
		<AIPromptEdit promptUse="branches" />
	</div>
</SettingsSection>

<style>
	.ai-settings__about-text {
		margin-bottom: 12px;
		color: var(--clr-text-2);
	}

	.prompt-groups {
		display: flex;
		flex-direction: column;
		margin-top: 16px;
		gap: 12px;
	}

	.ai-settings__short-note {
		display: flex;
		align-items: center;
		padding: 6px 10px;
		gap: 8px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.ai-settings__section-text-block {
		display: flex;
		flex-direction: column;
	}
</style>
