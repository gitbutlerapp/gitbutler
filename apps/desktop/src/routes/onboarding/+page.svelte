<script lang="ts">
	import analyticsSvg from '$lib/assets/illustrations/analytics.svg?raw';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Welcome from '$lib/components/Welcome.svelte';
	import { AppSettings } from '$lib/config/appSettings';
	import AnalyticsConfirmation from '$lib/settings/AnalyticsConfirmation.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const appSettings = getContext(AppSettings);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;
</script>

<DecorativeSplitView img={$analyticsConfirmed ? newProjectSvg : analyticsSvg}>
	{#if $analyticsConfirmed}
		<Welcome />
	{:else}
		<AnalyticsConfirmation />
	{/if}
</DecorativeSplitView>
