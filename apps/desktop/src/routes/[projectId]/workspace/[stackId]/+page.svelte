<script lang="ts">
	import StackView from '$components/v3/StackView.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);
	const stackId = $derived(page.params.stackId!);
</script>

{#if projectId && stackId}
	<StackView {projectId} {stackId} />
{/if}
