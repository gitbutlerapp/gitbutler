<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileViewPlaceholder from '$components/v3/FileViewPlaceholder.svelte';
	import SelectedChange from '$components/v3/SelectedChange.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { readKey, type SelectionId } from '$lib/selection/key';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
		draggableFiles?: boolean;
	};

	let { projectId, selectionId, draggableFiles }: Props = $props();

	const [idSelection] = inject(IdSelection);

	const selection = $derived(selectionId ? idSelection.values(selectionId) : []);
	const lastAdded = $derived(selectionId ? idSelection.getById(selectionId).lastAdded : undefined);

	const toPreview = $derived.by(() => {
		if (!selectionId) return;
		if (selection.length === 0) return;
		if (selection.length === 1 || !$lastAdded) return selection[0];
		return readKey($lastAdded.key);
	});
</script>

<div class="selection-view">
	{#if toPreview}
		<ScrollableContainer wide zIndex="var(--z-lifted)">
			<SelectedChange
				{projectId}
				selectedFile={toPreview}
				draggable={draggableFiles}
				onCloseClick={() => {
					if (selectionId) {
						idSelection.remove(toPreview.path, toPreview);
					}
				}}
			/>
		</ScrollableContainer>
	{:else}
		<FileViewPlaceholder />
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}
</style>
