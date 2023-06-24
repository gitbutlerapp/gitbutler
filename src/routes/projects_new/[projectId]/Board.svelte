<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { createEventDispatcher } from 'svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import { Commit, type Branch } from './types';
	import { plainToInstance } from 'class-transformer';

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

	const testCommits = plainToInstance(Commit, [
		{
			id: '1',
			authorEmail: 'mtsgrd@gmail.com',
			authorName: 'Mattias Granlund',
			description: 'Testing something out',
			createdAt: 1687538314,
			isRemote: true
		},
		{
			id: '2',
			authorEmail: 'kiril@videlov.com',
			authorName: 'Kiril Videlov',
			description: 'Testing something else out',
			createdAt: 1687538315,
			isRemote: true
		},
		{
			id: '3',
			authorEmail: 'nikita@galaiko.rocks',
			authorName: 'Nikita Galaiko',
			description: 'Rust rust rust',
			createdAt: 1687538316,
			isRemote: false
		},
		{
			id: '4',
			authorEmail: 'donahue.ian@gmail.com',
			authorName: 'Ian Donahue',
			description: 'Updated designs',
			createdAt: 1687538317,
			isRemote: false
		},
		{
			id: '5',
			authorEmail: 'schacon@gmail.com',
			authorName: 'Scott Chacon',
			description: 'Fixing that thing',
			createdAt: 1687538317,
			isRemote: false
		}
	]);
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
	{#each branches.filter((c) => c.active) as { id, name, files, description } (id)}
		<Lane
			bind:name
			bind:commitMessage={description}
			bind:files
			commits={testCommits}
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
