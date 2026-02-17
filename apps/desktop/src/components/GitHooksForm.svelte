<script lang="ts">
	import SettingsSection from "$components/SettingsSection.svelte";
	import { projectRunCommitHooks } from "$lib/config/config";
	import { CardGroup, Toggle } from "@gitbutler/ui";

	const { projectId }: { projectId: string } = $props();

	const runCommitHooks = $derived(projectRunCommitHooks(projectId));
</script>

<SettingsSection>
	<CardGroup>
		<CardGroup.Item labelFor="runHooks">
			{#snippet title()}
				Run Git hooks
			{/snippet}
			{#snippet caption()}
				Enabling this will run git pre-push, pre and post commit, and commit-msg hooks you have
				configured in your repository.
			{/snippet}
			{#snippet actions()}
				<Toggle id="runHooks" bind:checked={$runCommitHooks} />
			{/snippet}
		</CardGroup.Item>
	</CardGroup>
</SettingsSection>
