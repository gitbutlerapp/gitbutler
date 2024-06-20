<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import Checkbox from '$lib/shared/Checkbox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import type { Writable } from 'svelte/store';

	export let visible: boolean;
	export let showPrCheckbox: boolean;

	export let includePrs: Writable<boolean | undefined>;
	export let includeRemote: Writable<boolean | undefined>;
	export let includeStashed: Writable<boolean | undefined>;
	export let hideBots: Writable<boolean | undefined>;
	export let hideInactive: Writable<boolean | undefined>;
</script>

{#if visible}
	<ContextMenu>
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
{/if}
