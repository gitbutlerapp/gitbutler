<script lang="ts">
	import UnifiedDiffView from './v3/UnifiedDiffView.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { getContext } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const idSelection = getContext(IdSelection);
	const selection = $derived(idSelection.values());
</script>

<div class="selection-view">
	{#each selection as selectedFile (selectedFile.path)}
		<UnifiedDiffView {projectId} path={selectedFile.path} commitId={selectedFile.commitId} />
	{/each}
</div>

<style lang="postcss">
	.selection-view {
		padding: 12px;
	}
</style>
