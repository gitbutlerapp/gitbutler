<script lang="ts">
	import { Code, isUserErrorCode } from '$lib/backend/ipc';
	import ProjectNotFound from '$lib/components/ProjectNotFound.svelte';
	import SomethingWentWrong from '$lib/components/SomethingWentWrong.svelte';
	import { page } from '$app/stores';

	$: message = $page.error
		? $page.error.message
		: $page.status === 404
			? 'Page not found'
			: 'Unknown error';
</script>

{#if isUserErrorCode($page.error?.errorCode)}
	{#if $page.error?.errorCode === Code.ProjectMissing}
		<ProjectNotFound />
	{/if}
{:else}
	<SomethingWentWrong error={message} />
{/if}
