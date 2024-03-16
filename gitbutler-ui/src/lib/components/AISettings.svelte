<script lang="ts">
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import TextBox from './TextBox.svelte';
	import {
		AnthropicModelName,
		ConfigKeys,
		KeyOption,
		ModelKind,
		OpenAIModelName
	} from '$lib/backend/aiService';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getContextByClass } from '$lib/utils/context';
	import { onMount } from 'svelte';

	const gitConfigService = getContextByClass(GitConfigService);

	let modelKind: ModelKind;
	let openAIKeyOption: KeyOption;
	let anthropicKeyOption: KeyOption;
	let openAIKey: string | undefined;
	let openAIModelName: OpenAIModelName;
	let anthropicKey: string | undefined;
	let anthropicModelName: AnthropicModelName;

	$: gitConfigService.set('gitbutler.aiModelProvider', modelKind);

	$: gitConfigService.set('gitbutler.aiOpenAIKeyOption', openAIKeyOption);
	$: gitConfigService.set('gitbutler.aiOpenAIModelName', openAIModelName);
	$: if (openAIKey) gitConfigService.set('gitbutler.aiOpenAIKey', openAIKey);

	$: gitConfigService.set('gitbutler.aiAnthropicKeyOption', anthropicKeyOption);
	$: gitConfigService.set('gitbutler.aiAnthropicModelName', anthropicModelName);
	$: if (anthropicKey) gitConfigService.set('gitbutler.aiAnthropicKey', anthropicKey);

	onMount(async () => {
		modelKind = await gitConfigService.getWithDefault<ModelKind>(
			ConfigKeys.ModelProvider,
			ModelKind.OpenAI
		);

		openAIKeyOption = await gitConfigService.getWithDefault<KeyOption>(
			ConfigKeys.OpenAIKeyOption,
			KeyOption.ButlerAPI
		);
		openAIModelName = await gitConfigService.getWithDefault<OpenAIModelName>(
			ConfigKeys.OpenAIModelName,
			OpenAIModelName.GPT35Turbo
		);
		openAIKey = await gitConfigService.get(ConfigKeys.OpenAIKey);

		anthropicKeyOption = await gitConfigService.getWithDefault<KeyOption>(
			ConfigKeys.AnthropicKeyOption,
			KeyOption.ButlerAPI
		);
		anthropicModelName = await gitConfigService.getWithDefault<AnthropicModelName>(
			ConfigKeys.AnthropicModelName,
			AnthropicModelName.Haiku
		);
		anthropicKey = await gitConfigService.get(ConfigKeys.AnthropicKey);
	});

	$: if (form) form.modelKind.value = modelKind;

	const keyOptions = [
		{
			name: 'No, Use the GitButler API',
			value: KeyOption.ButlerAPI
		},
		{
			name: "Yes, I'll provide my own key",
			value: KeyOption.BringYourOwn
		}
	];

	const openAIModelOptions = [
		{
			name: 'GPT 3.5 Turbo',
			value: OpenAIModelName.GPT35Turbo
		},
		{
			name: 'GPT 4',
			value: OpenAIModelName.GPT4
		},
		{
			name: 'GPT 4 Turbo',
			value: OpenAIModelName.GPT4Turbo
		}
	];

	const anthropicModelOptions = [
		{
			name: 'Sonnet',
			value: AnthropicModelName.Sonnet
		},
		{
			name: 'Opus',
			value: AnthropicModelName.Opus
		},
		{
			name: 'Haiku',
			value: AnthropicModelName.Haiku
		}
	];

	let form: HTMLFormElement;

	function onFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		modelKind = formData.get('modelKind') as ModelKind;
	}
</script>

<p class="text-base-body-13 ai-settings__text">
	GitButler supports multiple providers for its AI powered features. We currently support models
	from OpenAI and Anthropic either proxied through the GitButler API, or in a bring your own key
	configuration.
</p>
<p class="text-base-body-13 ai-settings__text">
	To make use of the GitButler API you must be logged in
</p>

<form class="git-radio" bind:this={form} on:change={(e) => onFormChange(e.currentTarget)}>
	<SectionCard
		roundedBottom={false}
		orientation="row"
		labelFor="open-ai"
		bottomBorder={modelKind != ModelKind.OpenAI}
	>
		<svelte:fragment slot="title">Open AI</svelte:fragment>
		<svelte:fragment slot="actions">
			<RadioButton name="modelKind" id="open-ai" value={ModelKind.OpenAI} />
		</svelte:fragment>
		<svelte:fragment slot="caption">
			Leverage OpenAI's GPT models for branch name and commit message generation.
		</svelte:fragment>
	</SectionCard>
	{#if modelKind == ModelKind.OpenAI}
		<SectionCard hasTopRadius={false} roundedTop={false} roundedBottom={false} orientation="row">
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:selectedItemId={openAIKeyOption}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if openAIKeyOption == KeyOption.BringYourOwn}
					<TextBox label="OpenAI API Key" bind:value={openAIKey} required placeholder="sk-..." />

					<Select
						items={openAIModelOptions}
						bind:selectedItemId={openAIModelName}
						itemId="value"
						labelId="name"
						label="Model Version"
					>
						<SelectItem slot="template" let:item>
							{item.name}
						</SelectItem>
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
		bottomBorder={modelKind != ModelKind.Anthropic}
	>
		<svelte:fragment slot="title">Anthropic</svelte:fragment>
		<svelte:fragment slot="actions">
			<RadioButton name="modelKind" id="anthropic" value={ModelKind.Anthropic} />
		</svelte:fragment>
		<svelte:fragment slot="caption">
			Make use of Anthropic's Opus and Sonnet models for branch name and commit message generation.
		</svelte:fragment>
	</SectionCard>
	{#if modelKind == ModelKind.Anthropic}
		<SectionCard hasTopRadius={false} roundedTop={false} roundedBottom={false} orientation="row">
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:selectedItemId={anthropicKeyOption}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if anthropicKeyOption == KeyOption.BringYourOwn}
					<TextBox
						label="Anthropic API Key"
						bind:value={anthropicKey}
						required
						placeholder="sk-ant-api03-..."
					/>

					<Select
						items={anthropicModelOptions}
						bind:selectedItemId={anthropicModelName}
						itemId="value"
						labelId="name"
						label="Model Version"
					>
						<SelectItem slot="template" let:item>
							{item.name}
						</SelectItem>
					</Select>
				{/if}
			</div>
		</SectionCard>
	{/if}
	<SectionCard roundedTop={false} orientation="row">
		<svelte:fragment slot="title">Custom Endpoint</svelte:fragment>
		<svelte:fragment slot="actions">
			<RadioButton disabled={true} name="modelKind" />
		</svelte:fragment>
		<svelte:fragment slot="caption">Support for custom AI endpoints is coming soon!</svelte:fragment
		>
	</SectionCard>
</form>

<style>
	.ai-settings__text {
		color: var(--clr-theme-scale-ntrl-40);
	}

	.inputs-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
		width: 100%;
	}
</style>
