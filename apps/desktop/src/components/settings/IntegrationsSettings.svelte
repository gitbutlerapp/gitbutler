<script lang="ts">
	import BitbucketIntegration from "$components/settings/BitbucketIntegration.svelte";
	import GithubIntegration from "$components/settings/GithubIntegration.svelte";
	import GitlabIntegration from "$components/settings/GitlabIntegration.svelte";
	import { SETTINGS_SERVICE } from "$lib/settings/appSettings";
	import { inject } from "@gitbutler/core/context";
	import { CardGroup, Spacer, Toggle } from "@gitbutler/ui";

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

	async function toggleAutoFillPrDescription() {
		await settingsService.updateReviews({
			autoFillPrDescriptionFromCommit: !$appSettings?.reviews.autoFillPrDescriptionFromCommit,
		});
	}

	async function toggleStackFooter() {
		await settingsService.updateReviews({
			enableStackFooter: !($appSettings?.reviews.enableStackFooter ?? true),
		});
	}
</script>

<GithubIntegration />
<GitlabIntegration />
<BitbucketIntegration />
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
	<CardGroup.Item labelFor="enableStackFooter">
		{#snippet title()}
			Add GitButler stack footer to PR/MR descriptions
		{/snippet}
		{#snippet caption()}
			Append the list of pull requests in the same stack to the description.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="enableStackFooter"
				checked={$appSettings?.reviews.enableStackFooter ?? true}
				onclick={toggleStackFooter}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>
