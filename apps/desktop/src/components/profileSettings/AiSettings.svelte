<script lang="ts">
	import AIPromptEdit from '$components/AIPromptEdit.svelte';
	import AiCredentialCheck from '$components/AiCredentialCheck.svelte';
	import AuthorizationBanner from '$components/AuthorizationBanner.svelte';
	import Section from '$components/Section.svelte';
	import { AISecretHandle, AI_SERVICE, GitAIConfigKey, KeyOption } from '$lib/ai/service';
	import { OpenAIModelName, AnthropicModelName, ModelKind } from '$lib/ai/types';
	import { GIT_CONFIG_SERVICE } from '$lib/config/gitConfigService';
	import { SECRET_SERVICE } from '$lib/secrets/secretsService';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import {
		Icon,
		InfoMessage,
		Link,
		RadioButton,
		SectionCard,
		Select,
		SelectItem,
		Spacer,
		Textbox
	} from '@gitbutler/ui';

	import { onMount, tick } from 'svelte';
	import { run } from 'svelte/legacy';

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
			label: 'GPT 5',
			value: OpenAIModelName.GPT5
		},
		{
			label: 'GPT 5 Mini',
			value: OpenAIModelName.GPT5Mini
		},
		{
			label: 'o3 Mini',
			value: OpenAIModelName.O3mini
		},
		{
			label: 'o1 Mini',
			value: OpenAIModelName.O1mini
		},
		{
			label: 'GPT 4o mini',
			value: OpenAIModelName.GPT4oMini
		},
		{
			label: 'GPT 4.1',
			value: OpenAIModelName.GPT4_1
		},
		{
			label: 'GPT 4.1 mini (recommended)',
			value: OpenAIModelName.GPT4_1Mini
		}
	];

	const anthropicModelOptions = [
		{
			label: 'Haiku',
			value: AnthropicModelName.Haiku
		},
		{
			label: 'Sonnet 3.5',
			value: AnthropicModelName.Sonnet35
		},
		{
			label: 'Sonnet 3.7 (recommended)',
			value: AnthropicModelName.Sonnet37
		},
		{
			label: 'Sonnet 4',
			value: AnthropicModelName.Sonnet4
		},
		{
			label: 'Opus 4',
			value: AnthropicModelName.Opus4
		}
	];

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
	GitButler supports multiple AI providers: OpenAI and Anthropic (via API or your own key), plus
	local models through Ollama and LM Studio.
</p>

<form class="git-radio" bind:this={form} onchange={(e) => onFormChange(e.currentTarget)}>
	<SectionCard
		roundedBottom={false}
		orientation="row"
		labelFor="open-ai"
		bottomBorder={modelKind !== ModelKind.OpenAI}
	>
		{#snippet title()}
			Open AI
		{/snippet}
		{#snippet actions()}
			<RadioButton name="modelKind" id="open-ai" value={ModelKind.OpenAI} />
		{/snippet}
	</SectionCard>
	{#if modelKind === ModelKind.OpenAI}
		<SectionCard roundedTop={false} roundedBottom={false} topDivider>
			<Select
				value={openAIKeyOption}
				options={keyOptions}
				wide
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
					{@render shortNote('GitButler uses OpenAI API for commit messages and branch names.')}
				{/if}
			{/if}

			{#if openAIKeyOption === KeyOption.BringYourOwn}
				<Textbox
					label="API key"
					type="password"
					bind:value={openAIKey}
					required
					placeholder="sk-..."
				/>

				<Select
					value={openAIModelName}
					options={openAIModelOptions}
					label="Model version"
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
					label="Custom endpoint"
					bind:value={openAICustomEndpoint}
					placeholder="https://api.openai.com/v1"
				/>
			{/if}
		</SectionCard>
	{/if}

	<SectionCard
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
		labelFor="anthropic"
		bottomBorder={modelKind !== ModelKind.Anthropic}
	>
		{#snippet title()}
			Anthropic
		{/snippet}
		{#snippet actions()}
			<RadioButton name="modelKind" id="anthropic" value={ModelKind.Anthropic} />
		{/snippet}
	</SectionCard>
	{#if modelKind === ModelKind.Anthropic}
		<SectionCard roundedTop={false} roundedBottom={false} topDivider>
			<Select
				value={anthropicKeyOption}
				options={keyOptions}
				wide
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
					{@render shortNote('GitButler uses Anthropic API for commit messages and branch names.')}
				{/if}
			{/if}

			{#if anthropicKeyOption === KeyOption.BringYourOwn}
				<Textbox
					label="API key"
					type="password"
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
		</SectionCard>
	{/if}

	<SectionCard
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
		labelFor="ollama"
		bottomBorder={modelKind !== ModelKind.Ollama}
	>
		{#snippet title()}
			Ollama 🦙
		{/snippet}
		{#snippet actions()}
			<RadioButton name="modelKind" id="ollama" value={ModelKind.Ollama} />
		{/snippet}
	</SectionCard>
	{#if modelKind === ModelKind.Ollama}
		<SectionCard roundedTop={false} roundedBottom={false} topDivider>
			<Textbox label="Endpoint" bind:value={ollamaEndpoint} placeholder="http://127.0.0.1:11434" />
			<Textbox label="Model" bind:value={ollamaModel} placeholder="llama3" />
			<InfoMessage filled outlined={false}>
				{#snippet title()}
					Configuring Ollama
				{/snippet}
				{#snippet content()}
					To connect to your Ollama endpoint, <b>allow-list it in the app’s CSP settings</b>.
					<br />
					See the <Link href="https://docs.gitbutler.com/troubleshooting/custom-csp"
						>docs for details</Link
					>
				{/snippet}
			</InfoMessage>
		</SectionCard>
	{/if}

	<SectionCard
		roundedTop={false}
		roundedBottom={false}
		orientation="row"
		labelFor="lmstudio"
		bottomBorder={modelKind !== ModelKind.LMStudio}
	>
		{#snippet title()}
			LM Studio
		{/snippet}
		{#snippet actions()}
			<RadioButton name="modelKind" id="lmstudio" value={ModelKind.LMStudio} />
		{/snippet}
	</SectionCard>
	{#if modelKind === ModelKind.LMStudio}
		<SectionCard roundedTop={false} roundedBottom={false} topDivider>
			<Textbox label="Endpoint" bind:value={lmStudioEndpoint} placeholder="http://127.0.0.1:1234" />
			<Textbox label="Model" bind:value={lmStudioModel} placeholder="default" />
			<InfoMessage filled outlined={false}>
				{#snippet title()}
					Configuring LM Studio
				{/snippet}
				{#snippet content()}
					<div class="ai-settings__section-text-block">
						<p>Connecting to your LM Studio endpoint requires that you do two things:</p>

						<p>
							1. <span class="text-bold">Allow-list it in the CSP settings for the application</span
							>. You can find more details on how to do that in the <Link
								href="https://docs.gitbutler.com/troubleshooting/custom-csp">GitButler docs</Link
							>.
						</p>

						<p>
							2. <span class="text-bold">Enable CORS support in LM Studio</span>. You can find more
							details on how to do that in the <Link
								href="https://lmstudio.ai/docs/cli/server-start#enable-cors-support"
								>LM Studio docs</Link
							>.
						</p>
					</div>
				{/snippet}
			</InfoMessage>
		</SectionCard>
	{/if}

	<!-- AI credential check -->
	<SectionCard roundedTop={false}>
		<AiCredentialCheck />
	</SectionCard>
</form>

<Spacer />

<SectionCard orientation="row">
	{#snippet title()}
		Amount of provided context
	{/snippet}
	{#snippet caption()}
		How many characters of your git diff should be provided to AI
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
</SectionCard>
<Spacer />
<Section>
	{#snippet title()}
		Custom AI prompts
	{/snippet}
	{#snippet description()}
		GitButler's AI assistant generates commit messages and branch names. Use default prompts or
		create your own. Assign prompts in the project settings.
	{/snippet}

	<div class="prompt-groups">
		<AIPromptEdit promptUse="commits" />
		<Spacer margin={12} />
		<AIPromptEdit promptUse="branches" />
	</div>
</Section>

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
