<script lang="ts">
	import GithubIntegration from '$components/GithubIntegration.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { inject } from '@gitbutler/core/context';
	import { SectionCard, Spacer, Toggle } from '@gitbutler/ui';

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	async function toggleAutoFillPrDescription() {
		await settingsService.updateReviews({
			autoFillPrDescriptionFromCommit: !$appSettings?.reviews.autoFillPrDescriptionFromCommit
		});
	}
</script>

<p class="text-12 text-body header-settings__text">
	Credentails are persisted locally in your OS Keychain / Credential Manager.
</p>

<GithubIntegration />

<Spacer />

<SectionCard labelFor="autoFillPrDescription" orientation="row">
	{#snippet title()}
		Auto-fill PR descriptions from commit
	{/snippet}
	{#snippet caption()}
		When creating a pull request for a branch with just one commit, automatically use that commit's
		message as the PR title and description.
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="autoFillPrDescription"
			checked={$appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true}
			onclick={toggleAutoFillPrDescription}
		/>
	{/snippet}
</SectionCard>

<style>
	.header-settings__text {
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}
</style>
