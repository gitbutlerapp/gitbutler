<script lang="ts" async="true">
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { Branch } from './types';
	import type { VirtualBranchOperations } from './vbranches';

	export let projectId: string;
	export let branches: Branch[];
	export let virtualBranches: VirtualBranchOperations;

	const newBranchClass = 'new-branch-active';

	function handleDndEvent(e: CustomEvent<DndEvent<Branch>>) {
		branches = e.detail.items;

		if (e.type == 'finalize') {
			branches = branches.filter((branch) => branch.active);
			// ensure branch.order is sorted in ascending order
			// if not, send update requests
			branches.forEach((branch, i) => {
				if (branch.order !== i) {
					virtualBranches.updateBranchOrder(branch.id, i);
				}
			});
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
			{name}
			commitMessage={description}
			{files}
			{commits}
			on:empty={handleEmpty}
			{projectId}
			branchId={id}
			{virtualBranches}
		/>
	{/each}
	<NewBranchDropZone {branches} {virtualBranches} />
</div>

<style lang="postcss">
	:global(#branch-lanes.new-branch-active [data-dnd-ignore]) {
		@apply visible flex;
	}
</style>
