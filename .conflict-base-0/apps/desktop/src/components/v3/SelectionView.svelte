<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileViewPlaceholder from '$components/v3/FileViewPlaceholder.svelte';
	import IrcChannel from '$components/v3/IrcChannel.svelte';
	import SelectedChange from '$components/v3/SelectedChange.svelte';
	import { ircEnabled } from '$lib/config/uiFeatureFlags';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
		selectionId?: SelectionId;
	};

	let { projectId, selectionId }: Props = $props();

	const [idSelection, uiState] = inject(IdSelection, UiState);

	const channel = $derived(uiState.global.channel);

	const selection = $derived(
		selectionId ? idSelection.values(selectionId) : idSelection.values({ type: 'worktree' })
	);
</script>

<div class="selection-view">
	{#if selection.length === 0}
		{#if $ircEnabled && channel.current}
			{#if channel.current.startsWith('#')}
				<IrcChannel type="group" channel={channel.current} autojoin />
			{:else}
				<IrcChannel type="private" nick={channel.current} />
			{/if}
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
		height: 100%;
		overflow: hidden;
		background-size: 6px 6px;
		width: 100%;

		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
	}
</style>
