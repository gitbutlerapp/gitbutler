<script lang="ts">
	import { goto } from '$app/navigation';
	import AiPromptSelect from '$components/AIPromptSelect.svelte';
	import Section from '$components/Section.svelte';
	import WelcomeSigninAction from '$components/WelcomeSigninAction.svelte';
	import { projectAiExperimentalFeaturesEnabled, projectAiGenEnabled } from '$lib/config/config';
	import { focusable } from '$lib/focus/focusable';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { Button, SectionCard, Spacer, Toggle } from '@gitbutler/ui';

	const { projectId }: { projectId: string } = $props();

	const userService = inject(USER_SERVICE);
	const user = userService.user;

	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));
	const experimentalAiGenEnabled = $derived(projectAiExperimentalFeaturesEnabled(projectId));
</script>

<Section>
	{#snippet description()}
		GitButler supports the use of OpenAI and Anthropic to provide commit message and branch name
		generation. This works either through GitButler's API or in a bring your own key configuration
		and can be configured in the main preferences screen.
	{/snippet}

	<Spacer />

	{#if !$user}
		<WelcomeSigninAction />
		<Spacer />
	{/if}

	<div class="options">
		<SectionCard labelFor="aiGenEnabled" orientation="row" {focusable}>
			{#snippet title()}
				Enable branch and commit message generation
			{/snippet}
			{#snippet caption()}
				If enabled, diffs will be sent to OpenAI or Anthropic's servers when pressing the "Generate
				message" and "Generate branch name" button.
			{/snippet}
			{#snippet actions()}
				<Toggle
					id="aiGenEnabled"
					checked={$aiGenEnabled}
					onclick={() => {
						$aiGenEnabled = !$aiGenEnabled;
					}}
				/>
			{/snippet}
		</SectionCard>
	</div>

	{#if $aiGenEnabled}
		<div class="options">
			<SectionCard labelFor="aiExperimental" orientation="row" {focusable}>
				{#snippet title()}
					Enable experimental AI features
				{/snippet}
				{#snippet caption()}
					If enabled, you will be able to access the AI features currently in development. This also
					requires you to use OpenAI through GitButler in order for the features to work.
				{/snippet}
				{#snippet actions()}
					<Toggle
						id="aiExperimental"
						checked={$experimentalAiGenEnabled}
						onclick={() => {
							$experimentalAiGenEnabled = !$experimentalAiGenEnabled;
						}}
					/>
				{/snippet}
			</SectionCard>
		</div>
	{/if}

	<SectionCard {focusable}>
		{#snippet title()}
			Custom prompts
		{/snippet}

		<AiPromptSelect {projectId} promptUse="commits" />
		<AiPromptSelect {projectId} promptUse="branches" />

		<Spacer margin={8} />

		<p class="text-12 text-body">
			You can apply your own custom prompts to the project. By default, the project uses GitButler
			prompts, but you can create your own prompts in the general settings.
		</p>
		<Button kind="outline" icon="edit" onclick={() => goto(newSettingsPath('ai'))}
			>Customize prompts</Button
		>
	</SectionCard>
</Section>

<style lang="postcss">
	.options {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
