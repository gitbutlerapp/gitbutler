<script lang="ts">
	import Select from './Select.svelte';
	import SelectItem from './SelectItem.svelte';
	import TextBox from './TextBox.svelte';
	import {
		KeyOption,
		ModelKind,
		getModelKind,
		getKeyOption,
		setModelKind,
		setKeyOption,
		getAnthropicKey,
		setAnthropicKey,
		setOpenAIKey,
		getOpenAIKey,
		AnthropicModel,
		getAnthropicModel,
		OpenAIModel,
		getOpenAIModel,
		setAnthropicModel,
		setOpenAIModel
	} from '$lib/backend/summarizerSettings';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';

	let modelKind: ModelKind | undefined;
	getModelKind().then((persistedModelKind) => (modelKind = persistedModelKind));
	$: if (modelKind) setModelKind(modelKind);
	$: if (form && modelKind) form.modelKind.value = modelKind;

	let keyOption: { name: string; value: KeyOption } | undefined;
	getKeyOption().then(
		(persistedKeyOption) =>
			(keyOption = keyOptions.find((option) => option.value == persistedKeyOption))
	);
	$: if (keyOption) setKeyOption(keyOption.value);

	const keyOptions = [
		{
			name: 'No, Use the GitButler API',
			value: KeyOption.ButlerAPI
		},
		{
			name: "Yes, I'll use my own key",
			value: KeyOption.BringYourOwn
		}
	];

	let openAIKey: string | undefined;
	getOpenAIKey().then((persistedOpenAIKey) => (openAIKey = persistedOpenAIKey));
	$: if (openAIKey) setOpenAIKey(openAIKey);

	let openAIModel: { name: string; value: OpenAIModel } | undefined;
	getOpenAIModel().then(
		(persistedOpenAIModel) =>
			(openAIModel = openAIModelOptions.find(
				(option) => option.value == persistedOpenAIModel
			))
	);
	$: if (openAIModel) setOpenAIModel(openAIModel.value);

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

	let anthropicKey: string | undefined;
	getAnthropicKey().then((persistedAnthropicKey) => (anthropicKey = persistedAnthropicKey));
	$: if (anthropicKey) setAnthropicKey(anthropicKey);

	let anthropicModel: { name: string; value: AnthropicModel } | undefined;
	getAnthropicModel().then(
		(persistedAnthropicModel) =>
			(anthropicModel = anthropicModelOptions.find(
				(option) => option.value == persistedAnthropicModel
			))
	);
	$: if (anthropicModel) setAnthropicModel(anthropicModel.value);

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
		modelKind = formData.get('modelKind') as ModelKind;
	}
</script>

<p class="text-base-body-13 ai-settings__text">GitButler supports multiple models</p>

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
		<svelte:fragment slot="body">
			Leverage OpenAI's GPT models for branch name and commit message generation.
		</svelte:fragment>
	</SectionCard>
	{#if modelKind == ModelKind.OpenAI}
		<SectionCard
			hasTopRadius={false}
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
		>
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:value={keyOption}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if keyOption?.value === KeyOption.BringYourOwn}
					<TextBox
						label="OpenAI API Key"
						bind:value={openAIKey}
						required
						placeholder="sk-..."
					/>

					<Select
						items={openAIModelOptions}
						bind:value={openAIModel}
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
		<svelte:fragment slot="body">
			Make use of Anthropic's Opus and Sonnet models for branch name and commit message
			generation.
		</svelte:fragment>
	</SectionCard>
	{#if modelKind == ModelKind.Anthropic}
		<SectionCard
			hasTopRadius={false}
			roundedTop={false}
			roundedBottom={false}
			orientation="row"
		>
			<div class="inputs-group">
				<Select
					items={keyOptions}
					bind:value={keyOption}
					itemId="value"
					labelId="name"
					label="Do you want to provide your own key?"
				>
					<SelectItem slot="template" let:item>
						{item.name}
					</SelectItem>
				</Select>

				{#if keyOption?.value === KeyOption.BringYourOwn}
					<TextBox
						label="Anthropic API Key"
						bind:value={anthropicKey}
						required
						placeholder="sk-ant-api03-..."
					/>

					<Select
						items={anthropicModelOptions}
						bind:value={anthropicModel}
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
		<svelte:fragment slot="body">
			Support for custom AI endpoints is coming soon!
		</svelte:fragment>
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
