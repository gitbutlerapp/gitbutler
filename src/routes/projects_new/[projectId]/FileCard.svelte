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
	class="changed-file flex w-full flex-col justify-center gap-2 overflow-hidden rounded border-[0.5px] border-[#393939] bg-[#212121] p-2"
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
		class="flex flex-col gap-2"
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
					class="changed-hunk w-full rounded border-t-[0.5px] border-b-[0.5px] border-t-[#4C4C4C] border-b-[#252525] bg-[#2C2C2C] p-1"
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
