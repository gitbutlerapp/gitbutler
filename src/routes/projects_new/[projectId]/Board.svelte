<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';

	export let branches: Branch[];

	const flipDurationMs = 300;

	function handleDndEvent(
		e: CustomEvent<DndEvent<Branch | File | Hunk>> & { target: HTMLElement }
	) {
		branches = e.detail.items.filter((item) => item.kind == 'branch') as Branch[];
		// TODO: Create lanes out of dropped files/hunks
	}
</script>

<section
	class="flex gap-x-4 p-4"
	use:dndzone={{
		items: branches,
		flipDurationMs,
		types: ['branch'],
		receives: ['branch', 'file', 'hunk']
	}}
	on:consider={handleDndEvent}
	on:finalize={handleDndEvent}
>
	{#each branches.filter((c) => c.active) as { id, name, commits }, idx (id)}
		<div
			class="flex w-64 border border-zinc-700 bg-zinc-900/50 p-4"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} bind:commits />
		</div>
	{/each}
</section>
