<script lang="ts">
	import AnalyticsSettings from '$components/AnalyticsSettings.svelte';
	import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
	import { OnboardingEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { inject } from '@gitbutler/core/context';
	import { Button, TestId } from '@gitbutler/ui';

	const appSettings = inject(APP_SETTINGS);
	const posthog = inject(POSTHOG_WRAPPER);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-42">Before we begin</h1>
	<AnalyticsSettings />

	<div class="analytics-confirmation__actions">
		<Button
			style="pop"
			testId={TestId.OnboardingPageAnalyticsSettingsContinueButton}
			icon="chevron-right-small"
			onclick={() => {
				$analyticsConfirmed = true;
				initAnalyticsIfEnabled(appSettings, posthog, true).then(() => {
					// Await the initialization before logging the event to ensure PostHog is ready
					posthog.captureOnboarding(OnboardingEvent.ConfirmedAnalytics);
				});
			}}
		>
			Continue
		</Button>
	</div>
</div>

<style lang="postcss">
	.analytics-confirmation {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 12px;
	}

	.title {
		color: var(--clr-text-1);
	}

	.analytics-confirmation__actions {
		display: flex;
		justify-content: flex-end;
	}
</style>
