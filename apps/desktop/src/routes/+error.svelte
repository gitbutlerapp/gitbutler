<script lang="ts">
	import { page } from '$app/state';
	import ProjectNotFound from '$components/ProjectNotFound.svelte';
	import SomethingWentWrong from '$components/SomethingWentWrong.svelte';
	import { Code } from '$lib/error/knownErrors';

	const code = $derived(page.error?.errorCode);
	const status = $derived(page.status);
	const message = $derived(page.error?.message);

	const error = $derived(message ? message : status === 404 ? 'Page not found' : 'Unknown error');
</script>

{#if code === Code.ProjectMissing}
	<!-- We assume `projectId` is in the path given the code. -->
	{@const projectId = page.params.projectId!}
	<ProjectNotFound {projectId} />
{:else}
	<SomethingWentWrong {error} />
{/if}
