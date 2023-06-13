<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { Commit, File, Hunk } from './types';
	import CommitGroup from './CommitGroup.svelte';
	import { createEventDispatcher } from 'svelte';
	import { createCommit, createFile } from './helpers';

	export let name: string;
	export let commits: Commit[];

	const flipDurationMs = 150;
	const dispatch = createEventDispatcher();

	function handleDndEvent(e: CustomEvent<DndEvent<Commit | File | Hunk>>, isFinal: boolean) {
		const commitItems = e.detail.items.filter((item) => item.kind == 'commit') as Commit[];
		const fileItems = e.detail.items.filter((item) => item.kind == 'file') as File[];
		const hunkItems = e.detail.items.filter((item) => item.kind == 'hunk') as Hunk[];

		// Merge hunks into existing files, or create new where none exist
		for (const hunk of hunkItems) {
			commitItems.push(
				createCommit({
					files: [
						createFile({
							hunks: [{ ...hunk, isDndShadowItem: !isFinal }],
							isShadow: false,
							filePath: hunk.filePath
						})
					],
					isShadow: false
				})
			);
		}
		for (const file of fileItems) {
			commitItems.push(
				createCommit({ files: [{ ...file, isDndShadowItem: true }], isShadow: false })
			);
		}
		commits = commitItems.filter((commit) => commit.files && commit.files.length > 0);

		if (e.type == 'finalize' && (!commits || commits.length == 0)) {
			dispatch('empty');
		}
	}

	function handleEmpty() {
		const emptyIndex = commits.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			commits.splice(emptyIndex, 1);
		}
		if (commits.length == 0) {
			dispatch('empty');
		}
	}
</script>

<div class="flex h-full w-full flex-col">
	<div class="flex items-center justify-center p-4 font-bold">
		{name}
	</div>
	<div
		class="flex flex-grow flex-col gap-y-4"
		use:dndzone={{
			items: commits,
			flipDurationMs,
			zoneTabIndex: -1,
			types: ['commit'],
			receives: ['commit', 'file', 'hunk']
		}}
		on:consider={(e) => handleDndEvent(e, false)}
		on:finalize={(e) => handleDndEvent(e, true)}
	>
		{#each commits.filter((x) => x.files) as { id, description, files }, idx (id)}
			<div animate:flip={{ duration: flipDurationMs }}>
				<CommitGroup {id} bind:description bind:files on:empty={handleEmpty} />
			</div>
		{/each}
	</div>
</div>
