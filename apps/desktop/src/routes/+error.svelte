<script lang="ts">
	import { Code } from '$lib/backend/ipc';
	import ProjectNotFound from '$lib/components/ProjectNotFound.svelte';
	import SomethingWentWrong from '$lib/components/SomethingWentWrong.svelte';
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
