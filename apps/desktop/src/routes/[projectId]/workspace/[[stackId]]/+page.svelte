<script lang="ts">
	import SelectionView from '$components/SelectionView.svelte';
	import StackDetails from '$components/v3/StackDetails.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/state';

	const { data }: { data: PageData } = $props();

	const stackId = $derived(page.params.stackId);
	const projectId = $derived(data.projectId);
</script>

{#if stackId}
	<StackDetails {projectId} {stackId} />
{:else}
	<SelectionView {projectId} />
{/if}

<style>
</style>
