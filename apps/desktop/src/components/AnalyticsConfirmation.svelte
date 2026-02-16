<script lang="ts">
	import AnalyticsSettings from "$components/AnalyticsSettings.svelte";
	import { initAnalyticsIfEnabled } from "$lib/analytics/analytics";
	import { OnboardingEvent, POSTHOG_WRAPPER } from "$lib/analytics/posthog";
	import { SETTINGS_SERVICE } from "$lib/config/appSettingsV2";
	import { inject } from "@gitbutler/core/context";
	import { Button, TestId } from "@gitbutler/ui";

	const settingsService = inject(SETTINGS_SERVICE);
	const appSettings = $derived(settingsService.appSettings);
	const posthog = inject(POSTHOG_WRAPPER);
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-42">Before we begin</h1>
	<AnalyticsSettings />

	{#if $appSettings !== undefined}
		<div class="analytics-confirmation__actions">
			<Button
				style="pop"
				testId={TestId.OnboardingPageAnalyticsSettingsContinueButton}
				icon="chevron-right-small"
				onclick={() => {
					settingsService.updateOnboardingComplete(true);
					initAnalyticsIfEnabled($appSettings, posthog, true).then(() => {
						// Await the initialization before logging the event to ensure PostHog is ready
						posthog.captureOnboarding(OnboardingEvent.ConfirmedAnalytics);
					});
				}}
			>
				Continue
			</Button>
		</div>
	{/if}
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
