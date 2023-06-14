<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { Commit, File, Hunk } from './types';
	import CommitGroup from './CommitGroup.svelte';
	import { createEventDispatcher } from 'svelte';
	import { createCommit, createFile } from './helpers';

	export let name: string;
	export let commits: Commit[];

	const flipDurationMs = 150;
	const dispatch = createEventDispatcher();

	function handleDndEvent(e: CustomEvent<DndEvent<Commit | File | Hunk>>) {
		const newItems = e.detail.items;
		const commitItems = newItems.filter((item) => item instanceof Commit) as Commit[];

		const hunkItems = newItems.filter((item) => item instanceof Hunk) as Hunk[];
		for (const hunk of hunkItems) {
			commitItems.push(createCommit(createFile(hunk.filePath, hunk)));
		}

		const fileItems = newItems.filter((item) => item instanceof File) as File[];
		for (const file of fileItems) {
			commitItems.push(createCommit(file));
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
	<div class="flex items-center justify-center overflow-hidden p-4 font-bold">
		{name}
	</div>
	<div
		class="flex flex-grow flex-col gap-y-4 overflow-x-hidden overflow-y-scroll"
		use:dndzone={{
			items: commits,
			flipDurationMs,
			zoneTabIndex: -1,
			types: ['commit'],
			receives: ['commit', 'file', 'hunk']
		}}
		on:consider={handleDndEvent}
		on:finalize={handleDndEvent}
	>
		{#each commits.filter((x) => x.files) as { id, description, files }, idx (id)}
			<div class="w-full" animate:flip={{ duration: flipDurationMs }}>
				<CommitGroup {id} bind:description bind:files on:empty={handleEmpty} />
			</div>
		{/each}
	</div>
</div>
