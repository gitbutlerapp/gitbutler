<script lang="ts">
	import SeriesRowLabels from './SeriesLabels.svelte';
	import BranchLaneContextMenu from '$components/BranchLaneContextMenu.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import { PatchSeries } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		series: (PatchSeries | Error)[];
		onCollapseButtonClick: () => void;
	}

	const { series, onCollapseButtonClick }: Props = $props();

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonEl: HTMLButtonElement | undefined = $state();
	let isContextMenuOpen = $state(false);
</script>

<div class="stack-meta">
	<SeriesRowLabels {series} />

	<Button
		bind:el={kebabButtonEl}
		activated={isContextMenuOpen}
		kind="ghost"
		icon="kebab"
		size="tag"
		onclick={() => {
			contextMenu?.toggle();
		}}
	/>
	<BranchLaneContextMenu
		bind:contextMenuEl={contextMenu}
		trigger={kebabButtonEl}
		onCollapse={onCollapseButtonClick}
		ontoggle={(isOpen) => (isContextMenuOpen = isOpen)}
	/>
</div>

<style lang="postcss">
	.stack-meta {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top: none;
	}
</style>
