<script lang="ts">
	import { AIService, GitAIConfigKey, KeyOption } from '$lib/ai/service';
	import { OpenAIModelName, AnthropicModelName, ModelKind } from '$lib/ai/types';
	import { GitConfigService } from '$lib/backend/gitConfigService';
	import AiPromptEdit from '$lib/components/AIPromptEdit/AIPromptEdit.svelte';
	import InfoMessage from '$lib/components/InfoMessage.svelte';
	import RadioButton from '$lib/components/RadioButton.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import Select from '$lib/components/Select.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import Spacer from '$lib/components/Spacer.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import ContentWrapper from '$lib/settings/ContentWrapper.svelte';
	import Section from '$lib/settings/Section.svelte';
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
	let ollamaEndpoint: string | undefined;
	let ollamaModel: string | undefined;

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

	$: setConfiguration(GitAIConfigKey.OllamaEndpoint, ollamaEndpoint);
	$: setConfiguration(GitAIConfigKey.OllamaModelName, ollamaModel);

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
		},
		{
			name: 'GPT 4 Omni',
			value: OpenAIModelName.GPT4o
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
						items={keyOptions}
						bind:selectedItemId={openAIKeyOption}
						itemId="value"
						labelId="name"
						label="Do you want to provide your own key?"
					>
						<SelectItem
							slot="template"
							let:item
							let:selected
							{selected}
							let:highlighted
							{highlighted}
						>
							{item.name}
						</SelectItem>
					</Select>

					{#if openAIKeyOption === KeyOption.ButlerAPI}
						<InfoMessage filled outlined={false} style="pop" icon="ai">
							<svelte:fragment slot="title">
								GitButler uses OpenAI API for commit messages and branch names
							</svelte:fragment>
						</InfoMessage>
					{/if}

					{#if openAIKeyOption === KeyOption.BringYourOwn}
						<TextBox label="API key" bind:value={openAIKey} required placeholder="sk-..." />

						<Select
							items={openAIModelOptions}
							bind:selectedItemId={openAIModelName}
							itemId="value"
							labelId="name"
							label="Model version"
						>
							<SelectItem
								slot="template"
								let:item
								let:selected
								{selected}
								let:highlighted
								{highlighted}
							>
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
						items={keyOptions}
						bind:selectedItemId={anthropicKeyOption}
						itemId="value"
						labelId="name"
						label="Do you want to provide your own key?"
					>
						<SelectItem
							slot="template"
							let:item
							let:selected
							{selected}
							let:highlighted
							{highlighted}
						>
							{item.name}
						</SelectItem>
					</Select>

					{#if anthropicKeyOption === KeyOption.ButlerAPI}
						<InfoMessage filled outlined={false} style="pop" icon="ai">
							<svelte:fragment slot="title">
								GitButler uses Anthropic API for commit messages and branch names
							</svelte:fragment>
						</InfoMessage>
					{/if}

					{#if anthropicKeyOption === KeyOption.BringYourOwn}
						<TextBox
							label="API key"
							bind:value={anthropicKey}
							required
							placeholder="sk-ant-api03-..."
						/>

						<Select
							items={anthropicModelOptions}
							bind:selectedItemId={anthropicModelName}
							itemId="value"
							labelId="name"
							label="Model version"
						>
							<SelectItem
								slot="template"
								let:item
								let:selected
								{selected}
								let:highlighted
								{highlighted}
							>
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
					<TextBox
						label="Endpoint"
						bind:value={ollamaEndpoint}
						placeholder="http://127.0.0.1:11434"
					/>

					<TextBox label="Model" bind:value={ollamaModel} placeholder="llama3" />
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

	<Spacer />

	<Section>
		<svelte:fragment slot="title">Custom AI prompts</svelte:fragment>
		<svelte:fragment slot="description">
			GitButler's AI assistant generates commit messages and branch names. Use default prompts or
			create your own. Assign prompts in the <button
				class="link"
				on:click={() => console.log('got to project settings')}>project settings</button
			>.
		</svelte:fragment>

		<div class="prompt-groups">
			<AiPromptEdit promptUse="commits" />
			<Spacer margin={12} />
			<AiPromptEdit promptUse="branches" />
		</div>
	</Section>
</ContentWrapper>

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
