<script lang="ts">
	import ProjectNotFound from '$components/ProjectNotFound.svelte';
	import SomethingWentWrong from '$components/SomethingWentWrong.svelte';
	import { Code } from '$lib/backend/ipc';
	import { page } from '$app/stores';

	const code = $derived($page.error?.errorCode);
	const status = $derived($page.status);
	const message = $derived($page.error?.message);

	const error = $derived(message ? message : status === 404 ? 'Page not found' : 'Unknown error');
</script>

{#if code === Code.ProjectMissing}
	<ProjectNotFound />
{:else}
	<SomethingWentWrong {error} />
{/if}
