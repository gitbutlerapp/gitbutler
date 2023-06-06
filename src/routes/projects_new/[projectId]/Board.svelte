<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { Hunk, BranchLane } from './board';
	import Lane from './Lane.svelte';
	const flipDurationMs = 300;

	export let columns: BranchLane[];
	export let onFinalUpdate: (newColumns: BranchLane[]) => void;

	function handleDndConsiderColumns(e: { detail: { items: BranchLane[] } }) {
		columns = e.detail.items;
	}
	function handleDndFinalizeColumns(e: { detail: { items: BranchLane[] } }) {
		onFinalUpdate(e.detail.items);
	}
	function handleItemFinalize(columnIdx: number, newHunks: Hunk[]) {
		columns[columnIdx].hunks = newHunks;
		onFinalUpdate([...columns]);
	}
</script>

<section
	class="w-100"
	style="height: 90vh;"
	use:dndzone={{ items: columns, flipDurationMs, type: 'column' }}
	on:consider={handleDndConsiderColumns}
	on:finalize={handleDndFinalizeColumns}
>
	{#each columns as { id, name, hunks }, idx (id)}
		<div
			class="float-left m-2 flex h-full w-64 border border-zinc-700 bg-zinc-900/50 p-2"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} {hunks} onDrop={(newHunks) => handleItemFinalize(idx, newHunks)} />
		</div>
	{/each}
</section>
