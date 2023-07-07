<script lang="ts" async="true">
	import Lane from './BranchLane.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { Branch } from '$lib/api/ipc/vbranches';
	import type { VirtualBranchOperations } from './vbranches';

	export let projectId: string;
	export let projectPath: string;
	export let branches: Branch[];
	export let virtualBranches: VirtualBranchOperations;
	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	const hoverClass = 'drag-zone-hover';

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			branches.splice(emptyIndex, 1);
		}
		branches = branches;
	}
</script>

<div
	bind:this={dropZone}
	id="branch-lanes"
	class="flex max-w-full flex-shrink flex-grow snap-x items-start overflow-x-auto overflow-y-hidden bg-light-200 px-2 dark:bg-dark-1000"
	on:dragenter={(e) => {
		if (!e.dataTransfer?.types.includes('text/branch')) {
			return;
		}
		dropZone.classList.add(hoverClass);
	}}
	on:dragend={(e) => {
		if (!e.dataTransfer?.types.includes('text/branch')) {
			return;
		}
		dropZone.classList.remove(hoverClass);
	}}
	on:dragover={(e) => {
		if (!e.dataTransfer?.types.includes('text/branch')) {
			return;
		}
		e.preventDefault(); // Only when text/branch
		const children = [...e.currentTarget.children];
		dropPosition = 0;
		for (let i = 0; i < children.length; i++) {
			const pos = children[i].getBoundingClientRect();
			if (e.clientX > pos.left + pos.width) {
				dropPosition = i + 1; // Note that this is declared in the <script>
			} else {
				break;
			}
		}
		const idx = children.indexOf(dragged);
		if (idx != dropPosition) {
			idx >= dropPosition
				? children[dropPosition].before(dragged)
				: children[dropPosition].after(dragged);
		}
	}}
	on:drop={(e) => {
		dropZone.classList.remove(hoverClass);
		if (priorPosition != dropPosition) {
			const el = branches.splice(priorPosition, 1);
			branches.splice(dropPosition, 0, ...el);
			branches.forEach((branch, i) => {
				if (branch.order !== i) {
					virtualBranches.updateBranchOrder(branch.id, i);
				}
			});
		}
	}}
>
	{#each branches.filter((c) => c.active) as { id, name, files, commits, upstream, description, order } (id)}
		<Lane
			on:dragstart={(e) => {
				if (!e.dataTransfer) return;
				e.dataTransfer.setData('text/branch', id);
				dragged = e.currentTarget;
				priorPosition = Array.from(dropZone.children).indexOf(dragged);
			}}
			{name}
			commitMessage={description}
			{files}
			{commits}
			on:empty={handleEmpty}
			{order}
			{projectId}
			{upstream}
			branchId={id}
			{virtualBranches}
			{projectPath}
		/>
	{/each}
	<NewBranchDropZone {virtualBranches} />
</div>

<style lang="postcss">
	:global(.drag-zone-hover *) {
		@apply pointer-events-none;
	}
</style>
