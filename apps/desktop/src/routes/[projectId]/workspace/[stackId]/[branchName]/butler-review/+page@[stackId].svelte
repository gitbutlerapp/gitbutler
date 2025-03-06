<script lang="ts">
	import NewButlerReview from '$components/v3/NewButlerReview.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId);
	const branchName = $derived(page.params.branchName);
</script>

{#if projectId && stackId && branchName}
	<NewButlerReview {stackId} {projectId} {branchName} />
{/if}
