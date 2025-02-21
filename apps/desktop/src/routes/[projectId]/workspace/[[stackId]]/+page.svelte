<script lang="ts">
	import StackDetails from '$components/v3/StackDetails.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/state';

	const { data }: { data: PageData } = $props();

	const stackId = $derived(page.params.stackId);
	const projectId = $derived(data.projectId);
</script>

{#if stackId}
	<StackDetails {projectId} {stackId} />
{/if}
