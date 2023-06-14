<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';
	import { createEventDispatcher } from 'svelte';

	export let branches: Branch[];

	const flipDurationMs = 300;
	const dispatch = createEventDispatcher();

	function handleDndEvent(e: CustomEvent<DndEvent<Branch | File | Hunk>>) {
		const newItems = e.detail.items;
		const branchItems = newItems.filter((item) => item instanceof Branch) as Branch[];

		const hunkItems = newItems.filter((item) => item instanceof Hunk) as Hunk[];
		for (const hunk of hunkItems) {
			branchItems.push(createBranch(createFile(hunk.filePath, hunk)));
		}

		const fileItems = newItems.filter((item) => item instanceof File) as File[];
		for (const file of fileItems) {
			branchItems.push(createBranch(file));
		}

		branches = branchItems.filter((commit) => commit.active);

		if (e.type == 'finalize') {
			dispatch('finalize', branches);
		}
	}

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			// TODO: Figure out what to do when a branch is empty. Just removing it is a bit jarring.
		}
	}
</script>

<section
	class="swimlane-container flex h-full w-full gap-x-4 overflow-x-scroll bg-zinc-900 p-4"
	use:dndzone={{
		items: branches,
		flipDurationMs,
		types: ['branch'],
		receives: ['branch', 'commit', 'file', 'hunk']
	}}
	on:consider={handleDndEvent}
	on:finalize={handleDndEvent}
>
	{#each branches.filter((c) => c.active) as { id, name, files } (id)}
		<div
			class="swimlane flex h-full w-96 rounded-lg bg-zinc-900"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} bind:files on:empty={handleEmpty} />
		</div>
	{/each}
</section>
