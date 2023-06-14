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
	let expanded = true;

	function handleDndEvent(e: CustomEvent<DndEvent<Hunk>>) {
		file.hunks = e.detail.items;
		file.hunks.sort((itemA, itemB) => compareDesc(itemA.modifiedAt, itemB.modifiedAt));
		if (e.type == 'finalize' && file.hunks.length == 0) dispatch('empty');
	}
</script>

<div
	use:animateHeight
	class="flex w-full flex-col justify-center gap-2 overflow-hidden rounded border border-dotted border-zinc-600 bg-zinc-900 p-2"
>
	<button
		class="flex items-center gap-2 font-bold text-zinc-200"
		aria-expanded={expanded}
		on:click={() => (expanded = !expanded)}
	>
		<div>
			<svg width="16" height="16" viewBox="0 0 20 20" fill="none">
				<path class="vert" d="M10 1V19" stroke="currentColor" stroke-width="2" />
				<path d="M1 10L19 10" stroke="currentColor" stroke-width="2" />
			</svg>
		</div>
		<div class="overflow-hidden text-ellipsis whitespace-nowrap">
			{file.path}
		</div>
	</button>

	<div
		class="flex flex-col gap-1"
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
		{#if expanded}
			{#each file.hunks || [] as hunk (hunk.id)}
				<div
					animate:flip={{ duration: flipDurationMs }}
					class="w-full rounded border border-zinc-500 bg-zinc-600 p-1"
				>
					<div class="w-full text-ellipsis">
						{hunk.name}
					</div>
					<div class="text-right">
						{formatDistanceToNow(hunk.modifiedAt, { addSuffix: true })}
					</div>
				</div>
			{/each}
		{/if}
	</div>
</div>

<style>
	button[aria-expanded='true'] .vert {
		display: none;
	}
</style>
