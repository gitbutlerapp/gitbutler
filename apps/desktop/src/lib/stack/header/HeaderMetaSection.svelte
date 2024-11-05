<script lang="ts">
	import SeriesRowLabels from './SeriesRowLabels.svelte';
	import BranchLaneContextMenu from '$lib/branch/BranchLaneContextMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { PatchSeries } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		series: PatchSeries[];
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
		style="ghost"
		icon="kebab"
		size="tag"
		onclick={() => {
			contextMenu?.toggle();
		}}
	/>
	<BranchLaneContextMenu
		bind:contextMenuEl={contextMenu}
		target={kebabButtonEl}
		onCollapse={onCollapseButtonClick}
		onopen={() => (isContextMenuOpen = true)}
		onclose={() => (isContextMenuOpen = false)}
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
