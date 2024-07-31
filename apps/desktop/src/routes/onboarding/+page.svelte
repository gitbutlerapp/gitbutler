<script lang="ts">
	import analyticsSvg from '$lib/assets/illustrations/analytics.svg?raw';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Welcome from '$lib/components/Welcome.svelte';
	import { appAnalyticsConfirmed } from '$lib/config/appSettings';
	import AnalyticsConfirmation from '$lib/settings/AnalyticsConfirmation.svelte';

	const analyticsConfirmed = appAnalyticsConfirmed();
</script>

<DecorativeSplitView img={$analyticsConfirmed ? newProjectSvg : analyticsSvg}>
	{#if $analyticsConfirmed}
		<Welcome />
	{:else}
		<AnalyticsConfirmation {analyticsConfirmed} />
	{/if}
</DecorativeSplitView>
