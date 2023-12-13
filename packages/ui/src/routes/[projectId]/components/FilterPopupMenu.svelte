<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import Checkbox from '$lib/components/Checkbox.svelte';
	import Toggle from '$lib/components/Toggle.svelte';
	import type { Writable } from 'svelte/store';

	export let visible: boolean;

	export let includePrs: Writable<boolean>;
	export let includeRemote: Writable<boolean>;
	export let includeStashed: Writable<boolean>;
	export let hideBots: Writable<boolean>;
	export let hideInactive: Writable<boolean>;
</script>

{#if visible}
	<ContextMenu>
		<ContextMenuSection>
			<ContextMenuItem label="Pull requests">
				<Checkbox small bind:checked={$includePrs} slot="control" />
			</ContextMenuItem>

			<ContextMenuItem label="Remote">
				<Checkbox small bind:checked={$includeRemote} slot="control" />
			</ContextMenuItem>

			<ContextMenuItem label="Stashed">
				<Checkbox small bind:checked={$includeStashed} slot="control" />
			</ContextMenuItem>
		</ContextMenuSection>

		<ContextMenuSection>
			<ContextMenuItem label="Hide bots">
				<Toggle small slot="control" bind:checked={$hideBots} />
			</ContextMenuItem>
			<ContextMenuItem label="Hide inactive">
				<Toggle small slot="control" bind:checked={$hideInactive} />
			</ContextMenuItem>
		</ContextMenuSection>
	</ContextMenu>
{/if}

<style lang="postcss">
</style>
