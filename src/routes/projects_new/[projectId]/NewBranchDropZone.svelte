<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher();
	let items: Branch[] = [];

	function handleDndFinalize(e: CustomEvent<DndEvent<Branch | File | Hunk>>) {
		console.log(e);
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

		if (e.type == 'finalize') {
			dispatch('finalize', branchItems);
			items = [];
			return;
		}
		items = branchItems;
	}
</script>

<section
	id="new-branch-dz"
	class="invisible flex h-full items-center bg-zinc-900"
	use:dndzone={{
		items: items,
		types: ['new-branch'],
		receives: ['file', 'hunk'],
		dropTargetClassMap: {
			file: ['new-branch-active'],
			hunk: ['new-branch-active']
		}
	}}
	on:finalize={handleDndFinalize}
>
	<div
		class="h-full w-64 items-center rounded-lg border border-dashed bg-zinc-800 p-8 text-center text-xl font-bold"
	>
		drop here to create a new branch
	</div>
</section>

<style lang="postcss">
	:global(#new-branch-dz.new-branch-active) {
		@apply visible flex;
	}
</style>
