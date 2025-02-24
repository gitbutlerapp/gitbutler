<script lang="ts">
	import { goto } from '$app/navigation';
	import AnalyticsConfirmation from '$components/AnalyticsConfirmation.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import Welcome from '$components/Welcome.svelte';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import newZenSvg from '$lib/assets/illustrations/new-zen.svg?raw';
	import { APP_SETTINGS } from '$lib/config/appSettings';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { sleep } from '$lib/utils/sleep';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';

	const appSettings = inject(APP_SETTINGS);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;

	const projectsService = inject(PROJECTS_SERVICE);
	const projectsQuery = $derived(projectsService.projects());

	// We don't want to have this guard in the layout, because we want to have
	// `/onboarding/clone` accessible.
	$effect(() => {
		// Users should not be able to get here now that we load projects
		// sensibly, but hey, let's be sure.
		if (projectsQuery.response && projectsQuery.response.length > 0) {
			sleep(50).then(() => {
				goto('/');
			});
		}
	});
</script>

<DecorativeSplitView
	img={$analyticsConfirmed ? newZenSvg : newProjectSvg}
	testId={TestId.OnboardingPage}
>
	{#if $analyticsConfirmed}
		<Welcome />
	{:else}
		<AnalyticsConfirmation />
	{/if}
</DecorativeSplitView>
