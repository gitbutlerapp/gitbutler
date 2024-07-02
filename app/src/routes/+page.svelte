<script lang="ts">
	import analyticsSvg from '$lib/assets/illustrations/analytics.svg?raw';
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import { ProjectService } from '$lib/backend/projects';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import Welcome from '$lib/components/Welcome.svelte';
	import { appAnalyticsConfirmed } from '$lib/config/appSettings';
	import AnalyticsConfirmation from '$lib/settings/AnalyticsConfirmation.svelte';
	import { getContext } from '$lib/utils/context';
	import { derived } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const projectService = getContext(ProjectService);

	const projects = projectService.projects;

	$: debug = $page.url.searchParams.get('debug');

	const analyticsConfirmed = appAnalyticsConfirmed();
	const persistedId = projectService.getLastOpenedProject();
	const redirect = derived(projects, (projects) => {
		if (debug || !projects) return null;
		const projectId = projects.find((p) => p.id === persistedId)?.id;
		if (projectId) return projectId;
		if (projects.length > 0) return projects[0].id;
		return null;
	});

	$: if ($redirect) goto(`/${$redirect}/`);
</script>

{#if $redirect === undefined}
	<FullviewLoading />
{:else if !$analyticsConfirmed}
	<DecorativeSplitView img={analyticsSvg}>
		<AnalyticsConfirmation {analyticsConfirmed} />
	</DecorativeSplitView>
{:else if $redirect === null}
	<DecorativeSplitView img={newProjectSvg} showLinks={false}>
		<Welcome />
	</DecorativeSplitView>
{/if}
