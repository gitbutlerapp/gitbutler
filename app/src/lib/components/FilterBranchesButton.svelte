<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Checkbox from '$lib/shared/Checkbox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import type { Writable, Readable } from 'svelte/store';

	export let filtersActive: Readable<boolean>;
	export let showPrCheckbox: boolean;

	export let includePrs: Writable<boolean | undefined>;
	export let includeRemote: Writable<boolean | undefined>;
	export let includeStashed: Writable<boolean | undefined>;
	export let hideBots: Writable<boolean | undefined>;
	export let hideInactive: Writable<boolean | undefined>;

	let target: HTMLElement;
	let contextMenu: ContextMenu;

	export function onFilterClick() {
		contextMenu.toggle();
	}
</script>

<div class="header__filter-btn">
	<Button
		bind:el={target}
		style="ghost"
		outline
		icon={$filtersActive ? 'filter-applied-small' : 'filter-small'}
		on:mousedown={onFilterClick}
	>
		Filter
	</Button>
	<ContextMenu bind:this={contextMenu} {target}>
		<ContextMenuSection>
			{#if showPrCheckbox}
				<ContextMenuItem label="Pull requests" on:click={() => ($includePrs = !$includePrs)}>
					<Checkbox small bind:checked={$includePrs} slot="control" />
				</ContextMenuItem>
			{/if}
			<ContextMenuItem label="Remote" on:click={() => ($includeRemote = !$includeRemote)}>
				<Checkbox small bind:checked={$includeRemote} slot="control" />
			</ContextMenuItem>

			<ContextMenuItem label="Unapplied" on:click={() => ($includeStashed = !$includeStashed)}>
				<Checkbox small bind:checked={$includeStashed} slot="control" />
			</ContextMenuItem>
		</ContextMenuSection>

		<ContextMenuSection>
			<ContextMenuItem label="Hide bots" on:click={() => ($hideBots = !$hideBots)}>
				<Toggle small slot="control" bind:checked={$hideBots} />
			</ContextMenuItem>
			<ContextMenuItem label="Hide inactive" on:click={() => ($hideInactive = !$hideInactive)}>
				<Toggle small slot="control" bind:checked={$hideInactive} />
			</ContextMenuItem>
		</ContextMenuSection>
	</ContextMenu>
</div>
