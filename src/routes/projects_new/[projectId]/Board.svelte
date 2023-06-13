<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import Lane from './BranchLane.svelte';
	import type { Branch, Commit, File, Hunk } from './types';
	import type { DndEvent } from 'svelte-dnd-action/typings';

	export let branches: Branch[];

	const flipDurationMs = 300;

	function handleDndEvent(
		e: CustomEvent<DndEvent<Branch | Commit | File | Hunk>>,
		isFinal: boolean
	) {
		const branchItems = e.detail.items.filter((item) => item.kind == 'branch') as Branch[];
		const commitItems = e.detail.items.filter((item) => item.kind == 'commit') as Commit[];
		const fileItems = e.detail.items.filter((item) => item.kind == 'file') as File[];
		const hunkItems = e.detail.items.filter((item) => item.kind == 'hunk') as Hunk[];

		for (const hunk of hunkItems) {
			branchItems.push({
				id: `${Date.now()}-${hunk.id}-branch`,
				name: 'new branch',
				active: true,
				kind: 'branch',
				commits: [
					{
						id: `${Date.now()}-${hunk.id}-commit`,
						description: 'New commit',
						kind: 'commit',
						files: [
							{
								id: `${Date.now()}-${hunk.id}-hunk`,
								path: hunk.filePath,
								kind: 'file',
								hunks: [{ ...hunk, isDndShadowItem: !isFinal }]
							}
						]
					}
				]
			});
		}
		for (const file of fileItems) {
			branchItems.push({
				id: `${Date.now()}-${file.id}-branch`,
				name: 'new branch',
				active: true,
				kind: 'branch',
				commits: [
					{
						id: `${Date.now()}-${file.id}-commit`,
						description: '',
						kind: 'commit',
						files: [{ ...file, isDndShadowItem: false }],
						isDndShadowItem: !isFinal
					}
				]
			});
		}
		for (const commit of commitItems) {
			branchItems.push({
				id: `${Date.now()}-${commit.id}-branch`,
				name: 'new branch',
				kind: 'branch',
				active: true,
				commits: [commit],
				isDndShadowItem: !isFinal
			});
		}
		branches = branchItems.filter((commit) => commit.active);
	}

	function handleEmpty() {
		const emptyIndex = branches.findIndex((item) => !item.commits || item.commits.length == 0);
		if (emptyIndex != -1) {
			// TODO: Figure out what to do when a branch is empty. Just removing it is a bit jarring.
		}
	}
</script>

<section
	class="flex w-full gap-x-8 p-8"
	use:dndzone={{
		items: branches,
		flipDurationMs,
		types: ['branch'],
		receives: ['branch', 'commit', 'file', 'hunk']
	}}
	on:consider={(e) => handleDndEvent(e, false)}
	on:finalize={(e) => handleDndEvent(e, true)}
>
	{#each branches.filter((c) => c.active) as { id, name, commits }, idx (id)}
		<div
			class="flex w-64 border border-zinc-700 bg-zinc-900/50 p-4"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} bind:commits on:empty={handleEmpty} />
		</div>
	{/each}
</section>
