<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileViewPlaceholder from '$components/v3/FileViewPlaceholder.svelte';
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import SelectedChange from '$components/v3/SelectedChange.svelte';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		stackId?: string;
		branchName?: string;
	};

	let { projectId, stackId, branchName }: Props = $props();

	const [idSelection, uiState] = inject(IdSelection, UiState);

	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const selectionId = $derived(stackState?.activeSelectionId.get());
	const selection = $derived(selectionId?.current ? idSelection.values(selectionId.current) : []);
</script>

<div class="selection-view">
	{#if selection.length === 0}
		{#if $ircEnabled && branchName}
			<IrcChannel type="group" channel={'#' + branchName} autojoin />
		{:else}
			<FileViewPlaceholder />
		{/if}
	{:else}
		<ScrollableContainer wide>
			{#each selection as selectedFile}
				<SelectedChange
					{projectId}
					{selectedFile}
					onCloseClick={() => {
						if (selectionId) {
							idSelection.remove(selectedFile.path, selectionId.current);
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
		height: 100%;
		overflow: hidden;
		background-size: 6px 6px;
		width: 100%;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
	}
</style>
