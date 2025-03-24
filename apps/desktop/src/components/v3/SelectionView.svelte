<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileViewPlaceholder from '$components/v3/FileViewPlaceholder.svelte';
	import SelectedChange from '$components/v3/SelectedChange.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
	};

	const { projectId, stackId }: Props = $props();

	const [idSelection, uiState] = inject(IdSelection, UiState);
	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const selectionId = $derived(stackState?.activeSelectionId.get());
	const selection = $derived(selectionId?.current ? idSelection.values(selectionId.current) : []);
</script>

<div class="selection-view">
	{#if selection.length === 0}
		<FileViewPlaceholder />
	{:else}
		<ScrollableContainer wide>
			{#each selection as selectedFile}
				<SelectedChange {projectId} {selectedFile} />
			{/each}
		</ScrollableContainer>
	{/if}
</div>

<style>
	.selection-view {
		display: flex;
		flex-grow: 1;
		height: 100%;
		overflow: hidden;
		background-size: 6px 6px;
		width: 100%;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		align-items: center;
		justify-content: center;
	}
</style>
