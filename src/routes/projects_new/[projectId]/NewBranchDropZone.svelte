<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher();
	let items: Branch[] = [];

	function handleDndFinalize(e: CustomEvent<DndEvent<Branch | File | Hunk>>) {
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
			dispatch('newBranch', branchItems);
			items = [];
			return;
		}
		items = branchItems;
	}
</script>

<div
	id="new-branch-dz"
	class="invisible flex h-full items-center"
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
		class="flex h-full w-64 items-center self-center rounded-lg border border-dashed border-dark-100 bg-light-300 p-8 text-center text-xl font-bold dark:bg-dark-400"
	>
		drop here to create a new branch
	</div>
</div>

<style lang="postcss">
	:global(#new-branch-dz.new-branch-active) {
		@apply visible flex;
	}
</style>
