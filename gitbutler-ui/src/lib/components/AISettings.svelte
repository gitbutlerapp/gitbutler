<script lang="ts">
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import TextBox from './TextBox.svelte';
	import {
		AnthropicModel,
		KeyOption,
		ModelKind,
		OpenAIModel,
		SummarizerSettings
	} from '$lib/backend/summarizerSettings';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getContext } from 'svelte';

	let context = getContext<{ summarizerSettings: SummarizerSettings }>('page-context');

	let summarizerSettings = context.summarizerSettings;

	let modelKind$ = summarizerSettings.modelKind$;
	$: if (form) form.modelKind.value = $modelKind$;

	let keyOption$ = summarizerSettings.keyOption$;

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

	let openAIKey$ = summarizerSettings.openAIKey$;
	let openAIModel$ = summarizerSettings.openAIModel$;

	const openAIModelOptions = [
		{
			name: 'GPT 3.5 Turbo',
			value: OpenAIModel.GPT35Turbo
		},
		{
			name: 'GPT 4',
			value: OpenAIModel.GPT4
		},
		{
			name: 'GPT 4 Turbo',
			value: OpenAIModel.GPT4Turbo
		}
	];

	let anthropicKey$ = summarizerSettings.anthropicKey$;
	let anthropicModel$ = summarizerSettings.anthropicModel$;

	const anthropicModelOptions = [
		{
			name: 'Sonnet',
			value: AnthropicModel.Sonnet
		},
		{
			name: 'Opus',
			value: AnthropicModel.Opus
		}
	];

	let form: HTMLFormElement;

	function onFormChange(form: HTMLFormElement) {
		const formData = new FormData(form);
		$modelKind$ = formData.get('modelKind') as ModelKind;
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
		bottomBorder={$modelKind$ != ModelKind.OpenAI}
	>
		<svelte:fragment slot="title">Open AI</svelte:fragment>
		<svelte:fragment slot="actions">
			<RadioButton name="modelKind" id="open-ai" value={ModelKind.OpenAI} />
		</svelte:fragment>
		<svelte:fragment slot="caption">
			Leverage OpenAI's GPT models for branch name and commit message generation.
		</svelte:fragment>
	</SectionCard>
	{#if $modelKind$ == ModelKind.OpenAI}
		<SectionCard hasTopRadius={false} roundedTop={false} roundedBottom={false} orientation="row">
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:selectedItemId={$keyOption$}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if $keyOption$ == KeyOption.BringYourOwn}
					<TextBox label="OpenAI API Key" bind:value={$openAIKey$} required placeholder="sk-..." />

					<Select
						items={openAIModelOptions}
						bind:selectedItemId={$openAIModel$}
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
		bottomBorder={$modelKind$ != ModelKind.Anthropic}
	>
		<svelte:fragment slot="title">Anthropic</svelte:fragment>
		<svelte:fragment slot="actions">
			<RadioButton name="modelKind" id="anthropic" value={ModelKind.Anthropic} />
		</svelte:fragment>
		<svelte:fragment slot="caption">
			Make use of Anthropic's Opus and Sonnet models for branch name and commit message generation.
		</svelte:fragment>
	</SectionCard>
	{#if $modelKind$ == ModelKind.Anthropic}
		<SectionCard hasTopRadius={false} roundedTop={false} roundedBottom={false} orientation="row">
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:selectedItemId={$keyOption$}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if $keyOption$ == KeyOption.BringYourOwn}
					<TextBox
						label="Anthropic API Key"
						bind:value={$anthropicKey$}
						required
						placeholder="sk-ant-api03-..."
					/>

					<Select
						items={anthropicModelOptions}
						bind:selectedItemId={$anthropicModel$}
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
