<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import { derived as derivedStore } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	const projectsService = getContext(ProjectsService);

	const projects = projectsService.projects;

	const debug = $derived($page.url.searchParams.get('debug'));

	type Redirect =
		| {
				type: 'loading' | 'no-projects';
		  }
		| {
				type: 'redirect';
				subject: string;
		  };

	const persistedId = projectsService.getLastOpenedProject();
	const redirect = derivedStore(
		projects,
		(projects): Redirect => {
			if (debug) return { type: 'no-projects' };
			if (!projects) return { type: 'loading' };
			const projectId = projects.find((p) => p.id === persistedId)?.id;
			if (projectId) {
				return { type: 'redirect', subject: `/${projectId}` };
			}
			if (projects.length > 0) {
				return { type: 'redirect', subject: `/${projects[0]?.id}` };
			}
			return { type: 'no-projects' };
		},
		{ type: 'loading' } as Redirect
	);

	$effect(() => {
		if ($redirect.type === 'redirect') {
			goto($redirect.subject);
		} else if ($redirect.type === 'no-projects') {
			goto('/onboarding');
		}
	});
</script>

{#if $redirect.type === 'loading'}
	<FullviewLoading />
{/if}
