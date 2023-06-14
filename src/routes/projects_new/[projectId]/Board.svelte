<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';

	export let branches: Branch[];

	const flipDurationMs = 300;

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
		console.log(branches);
	}

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			// TODO: Figure out what to do when a branch is empty. Just removing it is a bit jarring.
		}
	}
</script>

<section
	class="flex h-full w-full gap-x-8 overflow-x-scroll p-2"
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
			class="flex h-full w-96 border border-zinc-700 bg-zinc-900/50 p-4"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} bind:files on:empty={handleEmpty} />
		</div>
	{/each}
</section>
