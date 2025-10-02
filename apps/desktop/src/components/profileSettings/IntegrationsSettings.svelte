<script lang="ts">
	import AuthorizationBanner from '$components/AuthorizationBanner.svelte';
	import GithubIntegration from '$components/GithubIntegration.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { SectionCard, Spacer, Toggle } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const user = userService.user;

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	async function toggleAutoFillPrDescription() {
		await settingsService.updateReviews({
			autoFillPrDescriptionFromCommit: !$appSettings?.reviews.autoFillPrDescriptionFromCommit
		});
	}
</script>

{#if !$user}
	<AuthorizationBanner />
	<Spacer />
{/if}
<GithubIntegration disabled={!$user} />

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
