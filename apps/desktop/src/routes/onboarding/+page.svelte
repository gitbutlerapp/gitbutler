<script lang="ts">
	import AnalyticsConfirmation from '$components/AnalyticsConfirmation.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import Welcome from '$components/Welcome.svelte';
	import analyticsSvg from '$lib/assets/illustrations/analytics.svg?raw';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { ProjectsService } from '$lib/backend/projects';
	import { AppSettings } from '$lib/config/appSettings';
	import { sleep } from '$lib/utils/sleep';
	import { getContext } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	const appSettings = getContext(AppSettings);
	const analyticsConfirmed = appSettings.appAnalyticsConfirmed;

	const projectsService = getContext(ProjectsService);
	const projects = projectsService.projects;

	// We don't want to have this guard in the layout, because we want to have
	// `/onboarding/clone` accessable.
	$effect(() => {
		// Users should not be able to get here now that we load projects
		// sensibly, but hey, let's be sure.
		if ($projects && $projects.length > 0) {
			sleep(50).then(() => {
				goto('/');
			});
		}
	});
</script>

<DecorativeSplitView img={$analyticsConfirmed ? newProjectSvg : analyticsSvg}>
	{#if $analyticsConfirmed}
		<Welcome />
	{:else}
		<AnalyticsConfirmation />
	{/if}
</DecorativeSplitView>
