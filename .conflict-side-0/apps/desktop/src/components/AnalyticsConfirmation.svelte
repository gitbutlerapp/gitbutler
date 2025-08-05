<script lang="ts">
	import AnalyticsSettings from '$components/AnalyticsSettings.svelte';
	import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
	import { POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import { Button } from '@gitbutler/ui';

	const appSettings = inject(APP_SETTINGS);
	const posthog = inject(POSTHOG_WRAPPER);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-40">Before we begin</h1>
	<AnalyticsSettings />

	<div class="analytics-confirmation__actions">
		<Button
			style="pop"
			testId={TestId.OnboardingPageAnalyticsSettingsContinueButton}
			icon="chevron-right-small"
			onclick={() => {
				$analyticsConfirmed = true;
				initAnalyticsIfEnabled(appSettings, posthog);
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
		color: var(--clr-scale-ntrl-0);
	}

	.analytics-confirmation__actions {
		display: flex;
		justify-content: flex-end;
	}
</style>
