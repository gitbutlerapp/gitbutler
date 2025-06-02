<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileViewPlaceholder from '$components/v3/FileViewPlaceholder.svelte';
	import SelectedChange from '$components/v3/SelectedChange.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
		draggableFiles: boolean;
	};

	let { projectId, selectionId, draggableFiles }: Props = $props();

	const [idSelection] = inject(IdSelection);

	const selection = $derived(selectionId ? idSelection.values(selectionId) : []);
</script>

<div class="selection-view">
	{#if selection.length === 0}
		<FileViewPlaceholder />
	{:else}
		<ScrollableContainer wide zIndex="var(--z-floating)">
			{#each selection as selectedFile}
				<SelectedChange
					{projectId}
					{selectedFile}
					draggable={draggableFiles}
					onCloseClick={() => {
						if (selectionId) {
							idSelection.remove(selectedFile.path, selectionId);
						}
					}}
				/>
			{/each}
		</ScrollableContainer>
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-ml);
		background-size: 6px 6px;
	}
</style>
