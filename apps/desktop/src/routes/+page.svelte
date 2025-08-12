<script lang="ts">
	import { goto } from '$app/navigation';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import { PROJECTS_SERVICE } from '$lib/project/projectsService';
	import { inject } from '@gitbutler/shared/context';

	const projectsService = inject(PROJECTS_SERVICE);

	const projectsResult = projectsService.projects();

	type Redirect =
		| {
				type: 'loading' | 'no-projects';
		  }
		| {
				type: 'redirect';
				subject: string;
		  };

	const persistedId = projectsService.getLastOpenedProject();
	const redirect: Redirect = $derived.by(() => {
		const projects = projectsResult.current.data;
		if (projects === undefined) return { type: 'loading' };
		const projectId = projects.find((p) => p.id === persistedId)?.id;
		if (projectId) {
			return { type: 'redirect', subject: `/${projectId}` };
		}
		if (projects.length > 0) {
			return { type: 'redirect', subject: `/${projects[0]?.id}` };
		}
		return { type: 'no-projects' };
	});

	$effect(() => {
		if (redirect.type === 'redirect') {
			goto(redirect.subject);
		} else if (redirect.type === 'no-projects') {
			goto('/onboarding');
		}
	});
</script>

{#if redirect.type === 'loading'}
	<FullviewLoading />
{/if}
