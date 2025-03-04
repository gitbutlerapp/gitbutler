<script lang="ts">
	import BranchLaneContextMenu from '$components/BranchLaneContextMenu.svelte';
	import SeriesRowLabels from '$components/SeriesLabels.svelte';
	import { PatchSeries } from '$lib/branches/branch';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';

	interface Props {
		series: (PatchSeries | Error)[];
		onCollapseButtonClick: () => void;
		stackId?: string;
	}

	const { series, onCollapseButtonClick }: Props = $props();

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabButtonEl: HTMLButtonElement | undefined = $state();
	let isContextMenuOpen = $state(false);
</script>

<div class="stack-meta">
	<div class="stack-meta-top">
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
</div>

<style lang="postcss">
	.stack-meta {
		width: 100%;
		align-items: start;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 4px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top: none;
	}

	.stack-meta-top {
		width: 100%;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
	}

	.stack-meta-bottom {
		width: 100%;
		display: flex;
		padding: 0 12px 12px 12px;
	}
</style>
