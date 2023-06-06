<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { FileCard } from './board';
	const flipDurationMs = 150;
	export let name: string;
	export let items: FileCard[];
	export let onDrop: (items: FileCard[]) => void;

	function handleDndConsiderCards(e: { detail: { items: FileCard[] } }) {
		console.warn('got consider', name);
		items = e.detail.items;
	}
	function handleDndFinalizeCards(e: { detail: { items: FileCard[] } }) {
		onDrop(e.detail.items);
	}
</script>

<div class="h-full w-full overflow-y-hidden">
	<div class="flex h-12 items-center justify-center font-bold">
		{name}
	</div>
	<div
		class="overflow-y-scroll"
		style="height: calc(100% - 2.5em);"
		use:dndzone={{ items, flipDurationMs, zoneTabIndex: -1 }}
		on:consider={handleDndConsiderCards}
		on:finalize={handleDndFinalizeCards}
	>
		{#each items as item (item.id)}
			<div
				class="my-2 flex h-14 w-full items-center justify-center rounded border border-zinc-600 bg-zinc-700"
			>
				{item.name}
			</div>
		{/each}
	</div>
</div>
