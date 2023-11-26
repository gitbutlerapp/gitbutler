<script lang="ts">
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { Branch } from '$lib/vbranches/types';
	import { slide } from 'svelte/transition';
	import FileCard from './FileCard.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { setExpandedWithCache } from './cache';
	import IconNewBadge from '$lib/icons/IconNewBadge.svelte';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';

	export let branch: Branch;
	export let selectable: boolean;
	export let projectPath: string | undefined;
	export let readonly: boolean;
	export let branchController: BranchController;
	export let selectedOwnership: Writable<Ownership>;
</script>

{#if branch.conflicted}
	<div class="mb-2 bg-red-500 p-2 font-bold text-white">
		{#if branch.files.some((f) => f.conflicted)}
			This virtual branch conflicts with upstream changes. Please resolve all conflicts and commit
			before you can continue.
		{:else}
			Please commit your resolved conflicts to continue.
		{/if}
	</div>
{/if}

<div class="flex flex-col">
	{#if branch.files.length > 0}
		<div
			class="flex flex-shrink flex-col gap-y-4 p-4"
			transition:slide={{ duration: readonly ? 0 : 250 }}
		>
			<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
			{#each sortLikeFileTree(branch.files) as file (file.id)}
				<FileCard
					expanded={file.expanded}
					conflicted={file.conflicted}
					{selectedOwnership}
					branchId={branch.id}
					{file}
					{projectPath}
					{branchController}
					{selectable}
					{readonly}
					on:expanded={(e) => {
						setExpandedWithCache(file, e.detail);
					}}
				/>
			{/each}
		</div>
	{/if}
	{#if branch.files.length == 0}
		{#if branch.commits.length == 0}
			<div class="no-changes text-color-3 space-y-6 rounded p-8 text-center" data-dnd-ignore>
				<p>Nothing on this branch yet.</p>
				{#if !readonly}
					<IconNewBadge class="mx-auto mt-4 h-16 w-16 text-blue-400" />
					<p class="px-12">Get some work done, then throw some files my way!</p>
				{/if}
			</div>
		{:else}
			<!-- attention: these markers have custom css at the bottom of thise file -->
			<div class="no-changes text-color-3 rounded py-6 text-center font-mono" data-dnd-ignore>
				No uncommitted changes on this branch
			</div>
		{/if}
	{/if}
</div>
