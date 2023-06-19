<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { File, Hunk } from './types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { createFile } from './helpers';
	import FileCard from './FileCard.svelte';

	export let name: string;
	export let description: string;
	export let files: File[];

	let descriptionHeight = 0;
	let textArea: HTMLTextAreaElement;
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

	function updateTextArea(): void {
		descriptionHeight = textArea.scrollHeight;
	}

	onMount(() => {
		updateTextArea();
	});
</script>

<div
	class="flex max-h-full w-96 shrink-0 flex-col overflow-y-hidden rounded-xl bg-light-700 px-1 dark:bg-dark-300"
>
	<div class="flex h-16 shrink-0 items-center px-3 text-lg font-bold">
		{name}
	</div>

	<div />
	<textarea
		bind:this={textArea}
		class="mx-1 mb-5 h-14 shrink-0 resize-none rounded border-0 bg-light-700 py-0 text-dark-400 focus-within:h-36 dark:bg-dark-300 dark:text-light-400"
		style="height: {descriptionHeight}px"
		value={description}
		on:change={updateTextArea}
	/>
	<div
		class="flex flex-shrink flex-col gap-y-2 overflow-y-auto rounded-lg px-1"
		use:dndzone={{
			items: files,
			zoneTabIndex: -1,
			types: ['file'],
			receives: ['file', 'hunk']
		}}
		on:consider={handleDndEvent}
		on:finalize={handleDndEvent}
	>
		{#each files.filter((x) => x.hunks) as file, idx (file.id)}
			<FileCard bind:file on:empty={handleEmpty} />
		{/each}
		<div
			data-dnd-ignore
			class="flex h-full w-full flex-col border-t border-light-200 p-2 dark:border-dark-200"
		>
			<div class="font-bold">Commits</div>
			<div>Commit 1</div>
			<div>Commit 2</div>
			<div>Commit 3</div>
			<div>Commit 1</div>
		</div>
	</div>
</div>
