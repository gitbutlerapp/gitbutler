<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import { Checkbox } from '$lib/components';
	import { writable } from 'svelte/store';
	import TimeAgo from '$lib/components/TimeAgo/TimeAgo.svelte';
	import IconChevronDownSmall from '$lib/icons/IconChevronDownSmall.svelte';
	import IconChevronRightSmall from '$lib/icons/IconChevronRightSmall.svelte';
	import IconFile from '$lib/icons/IconFile.svelte';
	import IconFolder from '$lib/icons/IconFolder.svelte';
	import { computeFileStatus, computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import { Ownership } from '$lib/vbranches/ownership';

	let className = '';
	export { className as class };
	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;
	export let withCheckboxes: boolean = false;
	export let selectedOwnership = writable(Ownership.default());

	function isNodeChecked(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			return node.file.hunks.some((hunk) => selectedOwnership.containsHunk(fileId, hunk.id));
		} else {
			return node.children.every((child) => isNodeChecked(selectedOwnership, child));
		}
	}

	$: isChecked = isNodeChecked($selectedOwnership, node);

	function isNodeIndeterminate(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			const numSelected = node.file.hunks.filter(
				(hunk) => !selectedOwnership.containsHunk(fileId, hunk.id)
			).length;
			return numSelected !== node.file.hunks.length && numSelected !== 0;
		}
		if (node.children.length === 0) return false;

		const isFirstNodeChecked = isNodeChecked(selectedOwnership, node.children[0]);
		const isFirstNodeIndeterminate = isNodeIndeterminate(selectedOwnership, node.children[0]);
		for (const child of node.children) {
			if (isFirstNodeChecked !== isNodeChecked(selectedOwnership, child)) {
				return true;
			}
			if (isFirstNodeIndeterminate !== isNodeIndeterminate(selectedOwnership, child)) {
				return true;
			}
		}
		return false;
	}

	$: isIndeterminate = isNodeIndeterminate($selectedOwnership, node);

	function idWithChildren(node: TreeNode): [string, string[]][] {
		if (node.file) {
			return [[node.file.id, node.file.hunks.map((h) => h.id)]];
		}
		return node.children.flatMap(idWithChildren);
	}

	function onCheckboxChange() {
		idWithChildren(node).forEach(([fileId, hunkIds]) =>
			hunkIds.forEach((hunkId) => {
				if (isChecked) {
					selectedOwnership.update((ownership) => ownership.removeHunk(fileId, hunkId));
				} else {
					selectedOwnership.update((ownership) => ownership.addHunk(fileId, hunkId));
				}
			})
		);
	}

	function toggle() {
		expanded = !expanded;
	}
</script>

<div class={className}>
	{#if isRoot}
		<!-- Node is a root and should only render children! -->
		<ul id={`fileTree-${fileTreeId++}`}>
			{#each node.children as childNode}
				<li>
					<svelte:self
						node={childNode}
						{selectedOwnership}
						{withCheckboxes}
						on:checked
						on:unchecked
					/>
				</li>
			{/each}
		</ul>
	{:else if node.file}
		{@const { added, removed } = computedAddedRemoved(node.file)}
		{@const status = computeFileStatus(node.file)}
		<!-- Node is a file -->
		<button
			class="flex w-full items-center gap-x-2 py-0 text-left"
			on:click={() => {
				const el = document.getElementById('file-' + node.file?.id);
				el?.scrollIntoView({ behavior: 'smooth' });
				setTimeout(() => el?.classList.add('wiggle'), 50);
				setTimeout(() => el?.classList.remove('wiggle'), 550);
			}}
		>
			<div class="w-4 shrink-0 text-center">
				<IconFile class="h-4 w-4" />
			</div>
			<div
				class="truncate"
				class:text-red-500={status == 'D'}
				class:dark:text-red-400={status == 'D'}
				class:text-green-700={status == 'A'}
				class:dark:text-green-500={status == 'A'}
				class:text-orange-800={status == 'M'}
				class:dark:text-orange-400={status == 'M'}
			>
				{node.name}
			</div>
			<div class="text-color-4 flex-grow truncate text-xs font-light">
				<TimeAgo date={node.file.modifiedAt} addSuffix={false} />
			</div>
			<div class="flex gap-1 font-mono text-xs font-bold">
				<span class="text-green-500">
					+{added}
				</span>
				<span class="text-red-500">
					-{removed}
				</span>
			</div>
			{#if withCheckboxes}
				<Checkbox
					checked={isChecked}
					indeterminate={isIndeterminate}
					on:change={onCheckboxChange}
				/>
			{/if}
		</button>
	{:else if node.children.length > 0}
		<!-- Node is a folder -->
		<button class="flex w-full items-center py-0 text-left" class:expanded on:click={toggle}>
			<div class="w-3 shrink-0 text-center">
				{#if expanded}
					<IconChevronDownSmall class="text-color-4 scale-90" />
				{:else}
					<IconChevronRightSmall class="text-color-4 scale-90" />
				{/if}
			</div>
			<div class="w-4 shrink-0 pl-1 text-center">
				<IconFolder class="h-4 w-4 scale-75 text-blue-400" />
			</div>
			<div class="flex-grow truncate pl-2">
				{node.name}
			</div>
			{#if withCheckboxes}
				<Checkbox
					checked={isChecked}
					indeterminate={isIndeterminate}
					on:change={onCheckboxChange}
				/>
			{/if}
		</button>
		<!-- We assume a folder cannot be empty -->
		{#if expanded}
			<div class="flex">
				<div class="flex">
					<div class="w-3 shrink-0 text-center">
						<div class="bg-color-3 inline-block h-full w-px" />
					</div>
				</div>
				<ul class="w-full overflow-hidden">
					{#each node.children as childNode}
						<li>
							<svelte:self
								node={childNode}
								expanded={true}
								{selectedOwnership}
								{withCheckboxes}
								on:checked
								on:unchecked
							/>
						</li>
					{/each}
				</ul>
			</div>
		{/if}
	{/if}
</div>
