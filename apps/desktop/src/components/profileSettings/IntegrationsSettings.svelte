<script lang="ts">
	import GithubIntegration from "$components/GithubIntegration.svelte";
	import GitlabIntegration from "$components/GitlabIntegration.svelte";
	import { SETTINGS_SERVICE } from "$lib/config/appSettingsV2";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Spacer, Toggle } from "@gitbutler/ui";

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	async function toggleAutoFillPrDescription() {
		await settingsService.updateReviews({
			autoFillPrDescriptionFromCommit: !$appSettings?.reviews.autoFillPrDescriptionFromCommit,
		});
	}
</script>

<GithubIntegration />
<GitlabIntegration />
<Spacer />
<CardGroup>
	<CardGroup.Item labelFor="autoFillPrDescription">
		{#snippet title()}
			Auto-fill PR/MR descriptions from commit
		{/snippet}
		{#snippet caption()}
			Set the title and description from the commit for single-commit branches.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="autoFillPrDescription"
				checked={$appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true}
				onclick={toggleAutoFillPrDescription}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>
