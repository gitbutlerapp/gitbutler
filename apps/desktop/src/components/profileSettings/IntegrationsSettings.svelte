<script lang="ts">
	import GithubIntegration from '$components/GithubIntegration.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const prService = $derived(forge.current.prService);
	const prUnit = $derived(prService?.unit);

	async function toggleAutoFillPrDescription() {
		await settingsService.updateReviews({
			autoFillPrDescriptionFromCommit: !$appSettings?.reviews.autoFillPrDescriptionFromCommit
		});
	}
</script>

<GithubIntegration />

<Spacer />

<CardGroup.Item labelFor="autoFillPrDescription">
	{#snippet title()}
		Auto-fill {prUnit?.abbr ?? 'PR'} descriptions from commit
	{/snippet}
	{#snippet caption()}
		When creating a {prUnit?.name.toLowerCase() ?? 'pull request'} for a branch with just one commit,
		automatically use that commit's message as the {prUnit?.abbr ?? 'PR'} title and description.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="autoFillPrDescription"
			checked={$appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true}
			onclick={toggleAutoFillPrDescription}
		/>
	{/snippet}
</CardGroup.Item>
