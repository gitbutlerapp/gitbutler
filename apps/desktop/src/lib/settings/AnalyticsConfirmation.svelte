<script lang="ts">
	import { initAnalyticsIfEnabled } from '$lib/analytics/analytics';
	import { PostHogWrapper } from '$lib/analytics/posthog';
	import { AppSettings } from '$lib/config/appSettings';
	import AnalyticsSettings from '$lib/settings/AnalyticsSettings.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const appSettings = getContext(AppSettings);
	const posthog = getContext(PostHogWrapper);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;
</script>

<div class="analytics-confirmation">
	<h1 class="title text-serif-40">Before we begin</h1>
	<AnalyticsSettings />

	<div class="analytics-confirmation__actions">
		<Button
			style="pop"
			kind="solid"
			testId="analytics-continue"
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
		width: 100%;
		display: flex;
		flex-direction: column;
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
