<script lang="ts">
	import BitbucketIntegration from "$components/settings/BitbucketIntegration.svelte";
	import GithubIntegration from "$components/settings/GithubIntegration.svelte";
	import GiteeIntegration from "$components/settings/GiteeIntegration.svelte";
	import GitlabIntegration from "$components/settings/GitlabIntegration.svelte";
	import { t } from "$lib/i18n";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Spacer, Toggle } from "@gitbutler/ui-svelte";

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
<GiteeIntegration />
<BitbucketIntegration />
<Spacer />
<CardGroup>
	<CardGroup.Item labelFor="autoFillPrDescription">
		{#snippet title()}
			{t("auto-fill-pr-mr-descriptions-from-commit")}
		{/snippet}
		{#snippet caption()}
			{t("set-the-title-and-description-from-the-commit-for-single-com")}
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
