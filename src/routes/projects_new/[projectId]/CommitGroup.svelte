<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { File, Hunk } from './types';
	import FileCard from './FileCard.svelte';
	import { createEventDispatcher } from 'svelte';
	import { createFile } from './helpers';

	export let description: string;
	export let id: string;
	export let files: File[];

	const flipDurationMs = 150;
	const dispatch = createEventDispatcher();

	function handleDndEvent(e: CustomEvent<DndEvent<File | Hunk>>, isFinal: boolean) {
		const fileItems = e.detail.items.filter((item) => item.kind == 'file') as File[];
		const hunkItems = e.detail.items.filter((item) => item.kind == 'hunk') as Hunk[];

		// Merge hunks into existing files, or create new where none exist
		for (const hunk of hunkItems) {
			const file = fileItems.find((file) => file.path == hunk.filePath);
			if (file) {
				file.hunks.push(hunk);
			} else {
				fileItems.push(
					createFile({
						filePath: hunk.filePath,
						hunks: [{ ...hunk, isDndShadowItem: !isFinal }],
						isShadow: false
					})
				);
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

<div class="flex w-full flex-col gap-y-2 border border-zinc-700 bg-zinc-900/70 p-2">
	<div
		class="flex w-full flex-col gap-y-2"
		id="commit-{id}"
		use:dndzone={{
			items: files,
			flipDurationMs,
			zoneTabIndex: -1,
			types: ['file'],
			receives: ['file', 'hunk']
		}}
		on:consider={(e) => handleDndEvent(e, false)}
		on:finalize={(e) => handleDndEvent(e, true)}
	>
		{#each files.filter((x) => x.hunks) as file, idx (file.id)}
			<div class="w-full" animate:flip={{ duration: flipDurationMs }}>
				<FileCard bind:file on:empty={handleEmpty} />
			</div>
		{/each}
	</div>
	<div>
		{description}
	</div>
</div>
