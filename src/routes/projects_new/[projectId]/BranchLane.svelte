<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { File, Hunk } from './types';
	import { createEventDispatcher } from 'svelte';
	import { createFile } from './helpers';
	import FileCard from './FileCard.svelte';

	export let name: string;
	export let files: File[];

	const flipDurationMs = 150;
	const dispatch = createEventDispatcher();

	function handleDndEvent(e: CustomEvent<DndEvent<File | Hunk>>) {
		const newItems = e.detail.items;
		const fileItems = newItems.filter((item) => item instanceof File) as File[];

		const hunkItems = newItems.filter((item) => item instanceof Hunk) as Hunk[];
		for (const hunk of hunkItems) {
			const file = fileItems.find((file) => file.path == hunk.filePath);
			if (file) {
				file.hunks.push(hunk);
			} else {
				fileItems.push(createFile(hunk.filePath, hunk));
			}
		}

		files = fileItems.filter((file) => file.hunks && file.hunks.length > 0);
		if (e.type == 'finalize' && files.length == 0) dispatch('empty');
	}

	function handleEmpty() {
		const emptyIndex = files.findIndex((item) => !item.hunks || item.hunks.length == 0);
		if (emptyIndex != -1) {
			files.splice(emptyIndex, 1);
		}
		if (files.length == 0) {
			dispatch('empty');
		}
	}
</script>

<div class="swimlane flex h-full w-full flex-col rounded-lg bg-[#2C2C2C] p-2">
	<div class="swimlane-header py-4 px-2 font-bold">
		{name}
	</div>
	<div
		class="commit-container overflow-auto rounded-lg border-[0.5px] border-[#393939] bg-[#212121] p-2"
	>
		<div class="commit-message py-2 text-lg font-bold">Commit message</div>
		<div
			class="flex w-full flex-grow flex-col gap-y-2 overflow-x-hidden overflow-y-scroll "
			use:dndzone={{
				items: files,
				flipDurationMs,
				zoneTabIndex: -1,
				types: ['file'],
				receives: ['file', 'hunk']
			}}
			on:consider={handleDndEvent}
			on:finalize={handleDndEvent}
		>
			{#each files.filter((x) => x.hunks) as file, idx (file.id)}
				<div class="changed-hunk w-full" animate:flip={{ duration: flipDurationMs }}>
					<FileCard bind:file on:empty={handleEmpty} />
				</div>
			{/each}
		</div>
	</div>
</div>
