<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';
	import { createEventDispatcher } from 'svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';

	export let branches: Branch[];

	const dispatch = createEventDispatcher();
	const newBranchClass = 'new-branch-active';

	function handleDndEvent(e: CustomEvent<DndEvent<Branch | File | Hunk>>) {
		if (e.type == 'consider' && !e.detail.info.types?.includes('branch')) {
			return; // No shadow element while considering drop.
		}
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

	function handleNewBranch(e: CustomEvent<Branch[]>) {
		branches.push(...e.detail);
	}

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			branches.splice(emptyIndex, 1);
		}
	}
</script>

<section
	id="branch-lanes"
	class="flex h-full snap-x gap-x-4 overflow-x-scroll bg-zinc-900 p-4"
	use:dndzone={{
		items: branches,
		types: ['branch'],
		receives: ['branch', 'file', 'hunk'],
		dropTargetClassMap: {
			file: [newBranchClass],
			hunk: [newBranchClass]
		}
	}}
	on:consider={handleDndEvent}
	on:finalize={handleDndEvent}
>
	{#each branches.filter((c) => c.active) as { id, name, files, description } (id)}
		<div
			id="branch-{id}"
			class="branch flex h-full w-96 snap-start scroll-ml-4 rounded-lg bg-zinc-900"
		>
			<Lane {name} {description} bind:files on:empty={handleEmpty} />
		</div>
	{/each}
	<NewBranchDropZone on:finalize={handleNewBranch} />
</section>

<style lang="postcss">
	:global(#branch-lanes.new-branch-active [data-dnd-ignore]) {
		@apply visible flex;
	}
</style>
