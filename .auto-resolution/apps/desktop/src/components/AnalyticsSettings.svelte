<script lang="ts">
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import { SectionCard, Toggle, Link } from '@gitbutler/ui';

	const appSettings = inject(APP_SETTINGS);
	const errorReportingEnabled = appSettings.appErrorReportingEnabled;
	const metricsEnabled = appSettings.appMetricsEnabled;
	const nonAnonMetricsEnabled = appSettings.appNonAnonMetricsEnabled;
</script>

<div class="analytics-settings__content">
	<p class="text-13 text-body analytics-settings__text">
		GitButler uses telemetry strictly to help us improve the client. We do not collect any personal
		information, unless explicitly allowed below (<Link
			target="_blank"
			rel="noreferrer"
			href="https://gitbutler.com/privacy"
		>
			privacy policy
		</Link>).
	</p>
	<p class="text-13 text-body analytics-settings__text">
		We kindly ask you to consider keeping these settings enabled as it helps us catch issues more
		quickly. If you choose to disable them, please feel free to share your feedback on our <Link
			target="_blank"
			rel="noreferrer"
			href="https://discord.gg/MmFkmaJ42D"
		>
			Discord
		</Link>.
	</p>
</div>

<div class="analytics-settings__actions" data-testid={TestId.OnboardingPageAnalyticsSettings}>
	<SectionCard labelFor="errorReportingToggle" orientation="row">
		{#snippet title()}
			Error reporting
		{/snippet}
		{#snippet caption()}
			Toggle reporting of application crashes and errors.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="errorReportingToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsErrorReportingToggle}
				checked={$errorReportingEnabled}
				onclick={() => ($errorReportingEnabled = !$errorReportingEnabled)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="metricsEnabledToggle" orientation="row">
		{#snippet title()}
			Usage metrics
		{/snippet}
		{#snippet caption()}
			Toggle sharing of usage statistics.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="metricsEnabledToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsTelemetryToggle}
				checked={$metricsEnabled}
				onclick={() => ($metricsEnabled = !$metricsEnabled)}
			/>
		{/snippet}
	</SectionCard>

	<SectionCard labelFor="nonAnonMetricsEnabledToggle" orientation="row">
		{#snippet title()}
			Non-anonymous usage metrics
		{/snippet}
		{#snippet caption()}
			Toggle sharing of identifiable usage statistics.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="nonAnonMetricsEnabledToggle"
				testId={TestId.OnboardingPageAnalyticsSettingsNonAnonymousToggle}
				checked={$nonAnonMetricsEnabled}
				onclick={() => ($nonAnonMetricsEnabled = !$nonAnonMetricsEnabled)}
			/>
		{/snippet}
	</SectionCard>
</div>

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

	.analytics-settings__actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
