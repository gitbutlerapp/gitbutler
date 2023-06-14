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

		if (e.type == 'finalize' && (!files || files.length == 0)) {
			dispatch('empty');
			return;
		}
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

<div class="flex h-full w-full flex-col">
	<div class="flex items-center justify-center overflow-hidden p-4 font-bold">
		{name}
	</div>
	<div
		class="flex w-full flex-grow flex-col gap-y-2 overflow-x-hidden overflow-y-scroll"
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
			<div class="w-full" animate:flip={{ duration: flipDurationMs }}>
				<FileCard bind:file on:empty={handleEmpty} />
			</div>
		{/each}
	</div>
</div>
