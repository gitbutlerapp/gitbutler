<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import AIPromptSelect from '$lib/components/AIPromptSelect.svelte';
	import SectionCard from '$lib/components/SectionCard.svelte';
	import WelcomeSigninAction from '$lib/components/WelcomeSigninAction.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import Section from '$lib/settings/Section.svelte';
	import { UserService } from '$lib/stores/user';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import { goto } from '$app/navigation';

	const userService = getContext(UserService);
	const project = getContext(Project);
	const user = userService.user;

	const aiGenEnabled = projectAiGenEnabled(project.id);
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
		<SectionCard labelFor="aiGenEnabled" orientation="row">
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

	<SectionCard>
		{#snippet title()}
			Custom prompts
		{/snippet}

		<AIPromptSelect promptUse="commits" />
		<AIPromptSelect promptUse="branches" />

		<Spacer margin={8} />

		<p class="text-12 text-body">
			You can apply your own custom prompts to the project. By default, the project uses GitButler
			prompts, but you can create your own prompts in the general settings.
		</p>
		<Button style="ghost" outline icon="edit-text" onclick={async () => await goto('/settings/ai')}
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
