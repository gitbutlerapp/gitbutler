<script lang="ts">
	import NewCommit from '$components/v3/NewCommit.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);
	const branchName = $derived(page.params.branchName);
	const commitId = $derived(page.url.searchParams.get('commitId') || undefined);
</script>

{#if projectId && stackId && branchName}
	<NewCommit {projectId} {stackId} {branchName} {commitId} />
{:else}
	Incorrect parameters.
{/if}
