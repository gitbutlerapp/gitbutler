<script lang="ts">
	import GithubIntegration from '$components/GithubIntegration.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, Spacer, Toggle } from '@gitbutler/ui';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = settingsService.appSettings;

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
		{$t('settings.general.integrations.autoFillPr.title')}
	{/snippet}
	{#snippet caption()}
		{$t('settings.general.integrations.autoFillPr.caption')}
	{/snippet}
	{#snippet actions()}
		<Toggle
			id="autoFillPrDescription"
			checked={$appSettings?.reviews.autoFillPrDescriptionFromCommit ?? true}
			onclick={toggleAutoFillPrDescription}
		/>
	{/snippet}
</CardGroup.Item>
