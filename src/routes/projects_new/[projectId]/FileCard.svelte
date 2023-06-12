<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { dndzone } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { formatDistanceToNow, compareDesc } from 'date-fns';
	import animateHeight from './animation';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { File, Hunk } from './types';

	export let file: File;

	const dispatch = createEventDispatcher();
	const flipDurationMs = 150;

	function handleDndEvent(e: CustomEvent<DndEvent<Hunk>>) {
		if (!file.hunks || file.hunks.length == 0) {
			dispatch('empty');
			return;
		}
		e.detail.items.sort((itemA, itemB) => compareDesc(itemA.modified, itemB.modified));
		file.hunks = e.detail.items;
	}
</script>

<div
	use:animateHeight
	class="w-fulljustify-center flex flex-col gap-2 rounded border border-zinc-600 bg-zinc-700 p-2"
>
	<div class="font-bold text-zinc-200">
		{file.path}
	</div>

	<div
		class="flex flex-col items-center gap-1"
		use:dndzone={{
			items: file.hunks,
			flipDurationMs,
			zoneTabIndex: -1,
			autoAriaDisabled: true,
			types: ['hunk', file.path],
			receives: [file.path]
		}}
		on:consider={handleDndEvent}
		on:finalize={handleDndEvent}
	>
		{#each file.hunks || [] as hunk (hunk.id)}
			<div
				animate:flip={{ duration: flipDurationMs }}
				class="w-full rounded border border-zinc-500 bg-zinc-600 p-1"
			>
				<div>
					{hunk.name}
				</div>
				<div class="text-right">
					{formatDistanceToNow(hunk.modified, { addSuffix: true })}
				</div>
			</div>
		{/each}
	</div>
</div>
