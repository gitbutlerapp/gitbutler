<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { dndzone } from 'svelte-dnd-action';
	import { flip } from 'svelte/animate';
	import { formatDistanceToNow, compareDesc } from 'date-fns';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { File, Hunk } from './types';
	import { Differ } from '$lib/components';
	import { line, type DiffArray } from '$lib/diff';
	import { diff } from '$lib';

	export let file: File;

	const dispatch = createEventDispatcher();
	const flipDurationMs = 150;
	let expanded = true;

	function handleDndEvent(e: CustomEvent<DndEvent<Hunk>>) {
		file.hunks = e.detail.items;
		file.hunks.sort((itemA, itemB) => compareDesc(itemA.modifiedAt, itemB.modifiedAt));
		if (e.type == 'finalize' && file.hunks.length == 0) dispatch('empty');
	}

	function diffStringToDiffArray(diffStr: string): DiffArray {
		let lines = diffStr.split('\n');
		let header = lines.shift();
		const before = lines.filter((line) => line.startsWith('-')).map((line) => line.slice(1));
		const after = lines.filter((line) => line.startsWith('+')).map((line) => line.slice(1));
		return line(before.slice(0, 2), after.slice(0, 2));
	}

	function diffLineNumberOffset(diffStr: string): number[] {
		let lines = diffStr.split('\n');
		let header = lines.shift();
		const lr = header?.split('@@')[1].trim().split(' ');
		if (!lr) return [0, 0];
		const before = lr[0].split(',')[0].slice(1);
		const after = lr[1].split(',')[0].slice(1);
		return [parseInt(before) + 2, parseInt(after) + 2];
	}

	function hunkSize(hunk: string): number[] {
		const linesAdded = hunk.split('\n').filter((line) => line.startsWith('+')).length;
		const linesRemoved = hunk.split('\n').filter((line) => line.startsWith('-')).length;
		return [linesAdded, linesRemoved];
	}
</script>

<div
	class="changed-file flex w-full flex-col justify-center gap-2 overflow-hidden bg-[#2C2C2C] p-2"
>
	<div class="flex items-center gap-2 font-bold text-zinc-200">
		<button
			class="cursor-pointer p-1"
			aria-expanded={expanded}
			on:click={() => (expanded = !expanded)}
		>
			<div>
				<svg width="16" height="16" viewBox="0 0 20 20" fill="none">
					<path class="vert" d="M10 1V19" stroke="currentColor" stroke-width="2" />
					<path d="M1 10L19 10" stroke="currentColor" stroke-width="2" />
				</svg>
			</div>
		</button>
		<div class="overflow-hidden text-ellipsis whitespace-nowrap">
			{file.path}
		</div>
	</div>

	<div
		class="hunk-change-container flex flex-col gap-2 rounded"
		use:dndzone={{
			items: file.hunks,
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
				<div class="changed-hunk flex w-full flex-col gap-1 rounded bg-[#212121] p-2">
					<div class="w-full text-ellipsis text-sm">
						{hunk.name}
					</div>
					<div class="cursor-pointer rounded border border-zinc-700 p-0.5 text-sm">
						<Differ
							diff={diffStringToDiffArray(hunk.diff)}
							lineNumberOffset={diffLineNumberOffset(hunk.diff)}
							filepath={hunk.filePath}
						/>
					</div>
					<div class="flex text-sm font-bold">
						<div class="flex flex-grow gap-1">
							<div class="text-green-600">+{hunkSize(hunk.diff)[0]}</div>
							{#if hunkSize(hunk.diff)[1] > 0}
								<div class="text-red-600">-{hunkSize(hunk.diff)[1]}</div>
							{/if}
						</div>
						<div class="text-right text-zinc-400">
							{formatDistanceToNow(hunk.modifiedAt, { addSuffix: true })}
						</div>
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
