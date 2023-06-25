<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import { Branch, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createBranch, createFile } from './helpers';
	import { createEventDispatcher } from 'svelte';
	import { Button } from '$lib/components';

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
	class="ml-4 mt-16 flex h-40 w-[22.5rem] shrink-0 items-center rounded-lg border border-dashed border-light-600 px-8 py-10"
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
		class="flex flex-col items-center gap-y-3 self-center p-8 text-center text-lg text-light-800 dark:text-dark-100"
	>
		<p>Drag changes or click button to create new virtual branch</p>
		<Button color="purple">New virtual branch</Button>
	</div>
</div>

<style lang="postcss">
	:global(#new-branch-dz.new-branch-active) {
		@apply visible flex;
	}
</style>
