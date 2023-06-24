<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createEventDispatcher } from 'svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import { type Branch, Target } from './types';

	export let branches: Branch[];
	export let projectId: string;

	const dispatch = createEventDispatcher();
	const newBranchClass = 'new-branch-active';

	function handleDndEvent(e: CustomEvent<DndEvent<Branch>>) {
		branches = e.detail.items.filter((branch) => branch.active);
		if (e.type == 'finalize') {
			dispatch('finalize', branches);
		}
	}

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			branches.splice(emptyIndex, 1);
		}
		branches = branches;
	}
</script>

<div
	id="branch-lanes"
	class="flex max-w-full flex-shrink flex-grow items-start overflow-x-auto overflow-y-hidden bg-light-200 px-2 dark:bg-dark-1000"
	use:dndzone={{
		items: branches,
		types: ['branch'],
		receives: ['branch'],
		dropTargetClassMap: {
			file: [newBranchClass],
			hunk: [newBranchClass]
		}
	}}
	on:consider={handleDndEvent}
	on:finalize={handleDndEvent}
>
	{#each branches.filter((c) => c.active) as { id, name, files, commits, description } (id)}
		<Lane
			bind:name
			bind:commitMessage={description}
			bind:files
			{commits}
			on:empty={handleEmpty}
			{projectId}
			branchId={id}
		/>
	{/each}
	<NewBranchDropZone on:newBranch />
</div>

<style lang="postcss">
	:global(#branch-lanes.new-branch-active [data-dnd-ignore]) {
		@apply visible flex;
	}
</style>
