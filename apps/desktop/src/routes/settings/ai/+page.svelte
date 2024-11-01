<script lang="ts">
	import { run } from 'svelte/legacy';

	import { AISecretHandle, AIService, GitAIConfigKey, KeyOption } from '$lib/ai/service';
	import { OpenAIModelName, AnthropicModelName, ModelKind } from '$lib/ai/types';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import AiPromptEdit from '$lib/components/AIPromptEdit/AIPromptEdit.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import SettingsPage from '$lib/layout/SettingsPage.svelte';
	import { getSecretsService } from '$lib/secrets/secretsService';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import AuthorizationBanner from '$lib/settings/AuthorizationBanner.svelte';
	import Section from '$lib/settings/Section.svelte';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import RadioButton from '$lib/shared/RadioButton.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { onMount, tick } from 'svelte';

	const gitConfigService = getContext(GitConfigService);
	const secretsService = getSecretsService();
	const aiService = getContext(AIService);
	const userService = getContext(UserService);
	const user = userService.user;
	let initialized = false;

	let modelKind: ModelKind | undefined = $state();
	let openAIKeyOption: KeyOption | undefined = $state();
	let anthropicKeyOption: KeyOption | undefined = $state();
	let openAIKey: string | undefined = $state();
	let openAIModelName: OpenAIModelName | undefined = $state();
	let anthropicKey: string | undefined = $state();
	let anthropicModelName: AnthropicModelName | undefined = $state();
	let diffLengthLimit: number | undefined = $state();
	let ollamaEndpoint: string | undefined = $state();
	let ollamaModel: string | undefined = $state();

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

		anthropicKeyOption = await aiService.getAnthropicKeyOption();
		anthropicModelName = await aiService.getAnthropicModelName();
		anthropicKey = await aiService.getAnthropicKey();

		diffLengthLimit = await aiService.getDiffLengthLimit();

		ollamaEndpoint = await aiService.getOllamaEndpoint();
		ollamaModel = await aiService.getOllamaModelName();

		// Ensure reactive declarations have finished running before we set initialized to true
		await tick();

		initialized = true;
	});

	const keyOptions = [
		{
			label: 'Use GitButler API',
			value: KeyOption.ButlerAPI
		},
		{
			label: 'Your own key',
			value: KeyOption.BringYourOwn
		}
	];

	const openAIModelOptions = [
		{
			label: 'GPT 3.5 Turbo',
			value: OpenAIModelName.GPT35Turbo
		},
		{
			label: 'GPT 4',
			value: OpenAIModelName.GPT4
		},
		{
			label: 'GPT 4 Turbo',
			value: OpenAIModelName.GPT4Turbo
		},
		{
			label: 'GPT 4o',
			value: OpenAIModelName.GPT4o
		},
		{
			label: 'GPT 4o mini (recommended)',
			value: OpenAIModelName.GPT4oMini
		}
	];

	const anthropicModelOptions = [
		{
			label: 'Sonnet',
			value: AnthropicModelName.Sonnet
		},
		{
			label: 'Opus',
			value: AnthropicModelName.Opus
		},
		{
			label: 'Haiku',
			value: AnthropicModelName.Haiku
		},
		{
			label: 'Sonnet 3.5 (recommended)',
			value: AnthropicModelName.Sonnet35
		}
	];

	let form: HTMLFormElement = $state();

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
		if (form) form.modelKind.value = modelKind;
	});
</script>

<SettingsPage title="AI options">
	<!-- <div class="ai-settings-wrap"> -->
	<p class="text-13 text-body ai-settings__text">
		GitButler supports multiple providers for its AI powered features. We currently support models
		from OpenAI and Anthropic either proxied through the GitButler API, or in a bring your own key
		configuration.
	</p>

	<form class="git-radio" bind:this={form} onchange={(e) => onFormChange(e.currentTarget)}>
		<SectionCard
			roundedBottom={false}
			orientation="row"
			labelFor="open-ai"
			bottomBorder={modelKind !== ModelKind.OpenAI}
		>
			<svelte:fragment slot="title">Open AI</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton name="modelKind" id="open-ai" value={ModelKind.OpenAI} />
			</svelte:fragment>
		</SectionCard>
		{#if modelKind === ModelKind.OpenAI}
			<SectionCard roundedTop={false} roundedBottom={false} orientation="row" topDivider>
				<div class="inputs-group">
					<Select
						value={openAIKeyOption}
						options={keyOptions}
						label="Do you want to provide your own key?"
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
							<AuthorizationBanner message="Please sign in to use the GitButler API." />
						{:else}
							<InfoMessage filled outlined={false} style="pop" icon="ai">
								<svelte:fragment slot="title">
									GitButler uses OpenAI API for commit messages and branch names
								</svelte:fragment>
							</InfoMessage>
						{/if}
					{/if}

					{#if openAIKeyOption === KeyOption.BringYourOwn}
						<Textbox label="API key" bind:value={openAIKey} required placeholder="sk-..." />

						<Select
							value={openAIModelName}
							options={openAIModelOptions}
							label="Model version"
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
					{/if}
				</div>
			</SectionCard>
		{/if}

		<SectionCard
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
			labelFor="anthropic"
			bottomBorder={modelKind !== ModelKind.Anthropic}
		>
			<svelte:fragment slot="title">Anthropic</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton name="modelKind" id="anthropic" value={ModelKind.Anthropic} />
			</svelte:fragment>
		</SectionCard>
		{#if modelKind === ModelKind.Anthropic}
			<SectionCard roundedTop={false} roundedBottom={false} orientation="row" topDivider>
				<div class="inputs-group">
					<Select
						value={anthropicKeyOption}
						options={keyOptions}
						label="Do you want to provide your own key?"
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
							<AuthorizationBanner message="Please sign in to use the GitButler API." />
						{:else}
							<InfoMessage filled outlined={false} style="pop" icon="ai">
								<svelte:fragment slot="title">
									GitButler uses Anthropic API for commit messages and branch names
								</svelte:fragment>
							</InfoMessage>
						{/if}
					{/if}

					{#if anthropicKeyOption === KeyOption.BringYourOwn}
						<Textbox
							label="API key"
							bind:value={anthropicKey}
							required
							placeholder="sk-ant-api03-..."
						/>

						<Select
							value={anthropicModelName}
							options={anthropicModelOptions}
							label="Model version"
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
				</div>
			</SectionCard>
		{/if}

		<SectionCard
			roundedTop={false}
			roundedBottom={modelKind !== ModelKind.Ollama}
			orientation="row"
			labelFor="ollama"
			bottomBorder={modelKind !== ModelKind.Ollama}
		>
			<svelte:fragment slot="title">Ollama 🦙</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton name="modelKind" id="ollama" value={ModelKind.Ollama} />
			</svelte:fragment>
		</SectionCard>
		{#if modelKind === ModelKind.Ollama}
			<SectionCard roundedTop={false} orientation="row" topDivider>
				<div class="inputs-group">
					<Textbox
						label="Endpoint"
						bind:value={ollamaEndpoint}
						placeholder="http://127.0.0.1:11434"
					/>

					<Textbox label="Model" bind:value={ollamaModel} placeholder="llama3" />
				</div>
			</SectionCard>
		{/if}
	</form>

	<Spacer />

	<SectionCard orientation="row">
		<svelte:fragment slot="title">Amount of provided context</svelte:fragment>
		<svelte:fragment slot="caption">
			How many characters of your git diff should be provided to AI
		</svelte:fragment>
		<svelte:fragment slot="actions">
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
		</svelte:fragment>
	</SectionCard>

	<Spacer />

	<Section>
		<svelte:fragment slot="title">Custom AI prompts</svelte:fragment>
		<svelte:fragment slot="description">
			GitButler's AI assistant generates commit messages and branch names. Use default prompts or
			create your own. Assign prompts in the <button
				type="button"
				class="link"
				onclick={() => console.log('got to project settings')}>project settings</button
			>.
		</svelte:fragment>

		<div class="prompt-groups">
			<AiPromptEdit promptUse="commits" />
			<Spacer margin={12} />
			<AiPromptEdit promptUse="branches" />
		</div>
	</Section>
</SettingsPage>

<style>
	.ai-settings__text {
		color: var(--clr-text-2);
		margin-bottom: 12px;
	}

	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: 16px;
		width: 100%;
	}

	.prompt-groups {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-top: 16px;
	}
</style>
