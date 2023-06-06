<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { Hunk } from './board';
	const flipDurationMs = 150;
	export let name: string;
	export let hunks: Hunk[];
	export let onDrop: (items: Hunk[]) => void;

	function handleDndConsiderCards(e: { detail: { items: Hunk[] } }) {
		console.warn('got consider', name);
		hunks = e.detail.items;
	}
	function handleDndFinalizeCards(e: { detail: { items: Hunk[] } }) {
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
		use:dndzone={{ items: hunks, flipDurationMs, zoneTabIndex: -1 }}
		on:consider={handleDndConsiderCards}
		on:finalize={handleDndFinalizeCards}
	>
		{#each hunks as hunk (hunk.id)}
			<div
				animate:flip={{ duration: flipDurationMs }}
				class="my-2 flex h-14 w-full items-center justify-center rounded border border-zinc-600 bg-zinc-700"
			>
				{hunk.description}
			</div>
		{/each}
	</div>
</div>
