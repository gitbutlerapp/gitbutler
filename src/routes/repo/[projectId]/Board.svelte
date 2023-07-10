<script lang="ts" async="true">
	import Lane from './BranchLane.svelte';
	import NewBranchDropZone from './NewBranchDropZone.svelte';
	import type { Branch } from '$lib/api/ipc/vbranches';
	import type { VirtualBranchOperations } from './vbranches';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { setContext } from 'svelte';
	import { dzHighlight } from './dropZone';

	export let projectId: string;
	export let projectPath: string;
	export let branches: Branch[];
	export let virtualBranches: VirtualBranchOperations;
	export let userSettings: SettingsStore;

	setContext(SETTINGS_CONTEXT, userSettings);

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
	class="flex max-w-full flex-shrink flex-grow snap-x items-start overflow-x-auto overflow-y-hidden bg-light-200 px-2 dark:bg-dark-1000"
	use:dzHighlight={{ type: dzType }}
	on:dragover={(e) => {
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
				e.dataTransfer.setData(dzType, id);
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
