<script lang="ts" async="true">
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createEventDispatcher, onMount } from 'svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import { Branch } from './types';
	import { plainToInstance } from 'class-transformer';
	import { invoke } from '$lib/ipc';
	import { getVBranchesOnBackendChange, sortBranchHunks } from './vbranches';
	import { error } from '$lib/toasts';

	export let projectId: string;
	export let branches: Branch[];

	getVBranchesOnBackendChange(projectId, (newBranches: Branch[]) => {
		branches = sortBranchHunks(newBranches);
	});

	const dispatch = createEventDispatcher();
	const newBranchClass = 'new-branch-active';

	async function getVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
		return invoke<Array<Branch>>('list_virtual_branches', params);
	}

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

	function updateBranches() {
		getVirtualBranches({ projectId })
			.then((res) => {
				branches = sortBranchHunks(plainToInstance(Branch, res));
			})
			.catch((e) => {
				console.log(e);
				error('Failed to update branch data');
			});
	}

	function handleUpdateRequest() {
		updateBranches();
	}

	onMount(() => {
		updateBranches();
	});
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
			on:update={handleUpdateRequest}
			{projectId}
			branchId={id}
		/>
	{/each}
	<NewBranchDropZone on:newBranch on:update={handleUpdateRequest} />
</div>

<style lang="postcss">
	:global(#branch-lanes.new-branch-active [data-dnd-ignore]) {
		@apply visible flex;
	}
</style>
