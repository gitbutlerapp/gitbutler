<script lang="ts">
	import NewPullRequest from '$components/v3/NewPullRequest.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);
	const branchName = $derived(page.params.branchName);
</script>

{#if projectId && stackId && branchName}
	<NewPullRequest {stackId} {projectId} {branchName} />
{/if}
