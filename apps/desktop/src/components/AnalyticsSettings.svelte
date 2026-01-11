<script lang="ts">
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { I18N_SERVICE } from '$lib/i18n/i18nService';
	import { inject } from '@gitbutler/core/context';
	import { CardGroup, TestId, Toggle } from '@gitbutler/ui';

	const i18nService = inject(I18N_SERVICE);
	const { t } = i18nService;
	const appSettings = inject(APP_SETTINGS);
	const errorReportingEnabled = appSettings.appErrorReportingEnabled;
	const metricsEnabled = appSettings.appMetricsEnabled;
	const nonAnonMetricsEnabled = appSettings.appNonAnonMetricsEnabled;
</script>

<div class="analytics-settings__content">
	<p class="text-13 text-body analytics-settings__text">
		{@html $t('settings.general.telemetry.description')}
	</p>
	<p class="text-13 text-body analytics-settings__text">
		{@html $t('settings.general.telemetry.request')}
	</p>
</div>

<CardGroup testId={TestId.OnboardingPageAnalyticsSettings}>
	<CardGroup.Item labelFor="errorReportingToggle">
		{#snippet title()}
			{$t('settings.general.telemetry.errorReporting.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.telemetry.errorReporting.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="errorReportingToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsErrorReportingToggle}
				checked={$errorReportingEnabled}
				onclick={() => ($errorReportingEnabled = !$errorReportingEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="metricsEnabledToggle">
		{#snippet title()}
			{$t('settings.general.telemetry.usageMetrics.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.telemetry.usageMetrics.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="metricsEnabledToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsTelemetryToggle}
				checked={$metricsEnabled}
				onclick={() => ($metricsEnabled = !$metricsEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>

	<CardGroup.Item labelFor="nonAnonMetricsEnabledToggle">
		{#snippet title()}
			{$t('settings.general.telemetry.nonAnonMetrics.title')}
		{/snippet}
		{#snippet caption()}
			{$t('settings.general.telemetry.nonAnonMetrics.caption')}
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="nonAnonMetricsEnabledToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsNonAnonymousToggle}
				checked={$nonAnonMetricsEnabled}
				onclick={() => ($nonAnonMetricsEnabled = !$nonAnonMetricsEnabled)}
			/>
		{/snippet}
	</CardGroup.Item>
</CardGroup>

<style lang="postcss">
	.analytics-settings__content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.analytics-settings__text {
		margin-bottom: 10px;
		color: var(--clr-text-2);
	}
</style>
