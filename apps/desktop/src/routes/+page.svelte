<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { derived } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const projectService = getContext(ProjectService);

	const projects = projectService.projects;

	$: debug = $page.url.searchParams.get('debug');

	const persistedId = projectService.getLastOpenedProject();
	const redirect = derived(projects, (projects) => {
		if (debug || !projects) return null;
		const projectId = projects.find((p) => p.id === persistedId)?.id;
		if (projectId) return projectId;
		if (projects.length > 0) return projects[0]?.id;
		return null;
	});

	$: {
		if ($redirect) {
			goto(`/${$redirect}/`);
		} else if ($redirect === null) {
			goto('/onboarding');
		}
	}
</script>

{#if $redirect === undefined}
	<FullviewLoading />
{/if}
