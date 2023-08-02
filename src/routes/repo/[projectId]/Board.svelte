<script lang="ts" async="true">
	import Lane from './BranchLane.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import { dzHighlight } from './dropZone';
	import { BRANCH_CONTROLLER_KEY, BranchController } from '$lib/vbranches/branchController';
	import { getContext } from 'svelte';
	import type { getCloudApiClient } from '$lib/api/cloud/api';
	export let projectId: string;
	export let projectPath: string;
	export let branches: Branch[];
	export let base: BaseBranch | undefined;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;

	const branchController = getContext<BranchController>(BRANCH_CONTROLLER_KEY);

	let dragged: any;
	let dropZone: HTMLDivElement;
	let priorPosition = 0;
	let dropPosition = 0;

	const dzType = 'text/branch';

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
	class="flex flex-shrink flex-grow items-start bg-light-150 dark:bg-dark-1000"
	role="group"
	use:dzHighlight={{ type: dzType }}
	on:dragover={(e) => {
		const children = [...e.currentTarget.children];
		dropPosition = 0;
		// We account for the NewBranchDropZone by subtracting 2
		for (let i = 0; i < children.length - 2; i++) {
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
	on:drop={() => {
		if (priorPosition != dropPosition) {
			const el = branches.splice(priorPosition, 1);
			branches.splice(dropPosition, 0, ...el);
			branches.forEach((branch, i) => {
				if (branch.order !== i) {
					branchController.updateBranchOrder(branch.id, i);
				}
			});
		}
	}}
>
	{#each branches.filter((c) => c.active) as { id, name, files, commits, order, conflicted, upstream } (id)}
		<Lane
			on:dragstart={(e) => {
				if (!e.dataTransfer) return;
				e.dataTransfer.setData(dzType, id);
				dragged = e.currentTarget;
				priorPosition = Array.from(dropZone.children).indexOf(dragged);
			}}
			{name}
			{files}
			{commits}
			{conflicted}
			on:empty={handleEmpty}
			{order}
			{projectId}
			branchId={id}
			{projectPath}
			{base}
			{cloudEnabled}
			{cloud}
			{upstream}
		/>
	{/each}
	<NewBranchDropZone />
</div>
