<script lang="ts">
	import { AIService, GitAIConfigKey, KeyOption, ModelKind } from '$lib/backend/aiService';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import { OpenAIModelName, AnthropicModelName } from '$lib/backend/types';
	import InfoMessage from '$lib/components/InfoMessage.svelte';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/components/Select.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import ContentWrapper from '$lib/components/settings/ContentWrapper.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '$lib/utils/context';
	import { onMount, tick } from 'svelte';

	const gitConfigService = getContext(GitConfigService);
	const aiService = getContext(AIService);
	const userService = getContext(UserService);
	const user = userService.user;

	let initialized = false;

	let modelKind: ModelKind | undefined;
	let openAIKeyOption: KeyOption | undefined;
	let anthropicKeyOption: KeyOption | undefined;
	let openAIKey: string | undefined;
	let openAIModelName: OpenAIModelName | undefined;
	let anthropicKey: string | undefined;
	let anthropicModelName: AnthropicModelName | undefined;
	let diffLengthLimit: number | undefined;

	function setConfiguration(key: GitAIConfigKey, value: string | undefined) {
		if (!initialized) return;

		gitConfigService.set(key, value || '');
	}

	$: setConfiguration(GitAIConfigKey.ModelProvider, modelKind);

	$: setConfiguration(GitAIConfigKey.OpenAIKeyOption, openAIKeyOption);
	$: setConfiguration(GitAIConfigKey.OpenAIModelName, openAIModelName);
	$: setConfiguration(GitAIConfigKey.OpenAIKey, openAIKey);

	$: setConfiguration(GitAIConfigKey.AnthropicKeyOption, anthropicKeyOption);
	$: setConfiguration(GitAIConfigKey.AnthropicModelName, anthropicModelName);
	$: setConfiguration(GitAIConfigKey.AnthropicKey, anthropicKey);
	$: setConfiguration(GitAIConfigKey.DiffLengthLimit, diffLengthLimit?.toString());

	onMount(async () => {
		modelKind = await aiService.getModelKind();

		openAIKeyOption = await aiService.getOpenAIKeyOption();
		openAIModelName = await aiService.getOpenAIModleName();
		openAIKey = await aiService.getOpenAIKey();

		anthropicKeyOption = await aiService.getAnthropicKeyOption();
		anthropicModelName = await aiService.getAnthropicModelName();
		anthropicKey = await aiService.getAnthropicKey();

		diffLengthLimit = await aiService.getDiffLengthLimit();

		// Ensure reactive declarations have finished running before we set initialized to true
		await tick();

		initialized = true;
	});

	$: if (form) form.modelKind.value = modelKind;

	const keyOptions = [
		{
			name: 'Use GitButler API',
			value: KeyOption.ButlerAPI
		},
		{
			name: 'Your own key',
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

<ContentWrapper title="AI options">
	<!-- <div class="ai-settings-wrap"> -->
	<p class="text-base-body-13 ai-settings__text">
		GitButler supports multiple providers for its AI powered features. We currently support models
		from OpenAI and Anthropic either proxied through the GitButler API, or in a bring your own key
		configuration.
	</p>

	{#if !$user}
		<InfoMessage>
			<svelte:fragment slot="title">You must be logged in to use the GitButler API</svelte:fragment>
		</InfoMessage>
	{/if}

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
		</SectionCard>
		{#if modelKind == ModelKind.OpenAI}
			<SectionCard
				hasTopRadius={false}
				roundedTop={false}
				roundedBottom={false}
				orientation="row"
				topDivider
			>
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

					{#if openAIKeyOption == KeyOption.ButlerAPI}
						<InfoMessage filled outlined={false} style="pop" icon="ai">
							<svelte:fragment slot="title">
								GitButler uses OpenAI API for commit messages and branch names
							</svelte:fragment>
						</InfoMessage>
					{/if}

					{#if openAIKeyOption == KeyOption.BringYourOwn}
						<TextBox label="API Key" bind:value={openAIKey} required placeholder="sk-..." />

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
					{:else if !$user}
						<WelcomeSigninAction prompt="A user is required to make use of the GitButler API" />
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
		</SectionCard>
		{#if modelKind == ModelKind.Anthropic}
			<SectionCard
				hasTopRadius={false}
				roundedTop={false}
				roundedBottom={false}
				orientation="row"
				topDivider
			>
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

					{#if anthropicKeyOption == KeyOption.ButlerAPI}
						<InfoMessage filled outlined={false} style="pop" icon="ai">
							<svelte:fragment slot="title">
								GitButler uses Anthropic API for commit messages and branch names
							</svelte:fragment>
						</InfoMessage>
					{/if}

					{#if anthropicKeyOption == KeyOption.BringYourOwn}
						<TextBox
							label="API Key"
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
					{:else if !$user}
						<WelcomeSigninAction prompt="A user is required to make use of the GitButler API" />
					{/if}
				</div>
			</SectionCard>
		{/if}

		<SectionCard roundedTop={false} orientation="row" disabled={true}>
			<svelte:fragment slot="title">Custom Endpoint</svelte:fragment>
			<svelte:fragment slot="actions">
				<RadioButton disabled={true} name="modelKind" />
			</svelte:fragment>
			<svelte:fragment slot="caption"
				>Support for custom AI endpoints is coming soon!</svelte:fragment
			>
		</SectionCard>
	</form>

	<Spacer />

	<SectionCard orientation="row">
		<svelte:fragment slot="title">Amount of provided context</svelte:fragment>
		<svelte:fragment slot="caption">
			How many characters of your git diff should be provided to AI
		</svelte:fragment>
		<svelte:fragment slot="actions">
			<TextBox
				type="number"
				width={80}
				textAlign="center"
				value={diffLengthLimit?.toString()}
				minVal={100}
				on:input={(e) => {
					diffLengthLimit = parseInt(e.detail);
				}}
				placeholder="5000"
			/>
		</svelte:fragment>
	</SectionCard>

	<style>
		.ai-settings__text {
			color: var(--clr-scale-ntrl-40);
			margin-bottom: var(--size-12);
		}

		.inputs-group {
			display: flex;
			flex-direction: column;
			gap: var(--size-16);
			width: 100%;
		}
	</style>
</ContentWrapper>
