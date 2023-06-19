<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { dndzone } from 'svelte-dnd-action';
	import { formatDistanceToNow, compareDesc } from 'date-fns';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { Hunk } from './types';
	import { Differ } from '$lib/components';
	import { line, type DiffArray } from '$lib/diff';

	export let filepath: string;
	export let hunks: Hunk[];
	let zoneEl: HTMLElement;

	const dispatch = createEventDispatcher();
	let expanded = true;

	function handleDndEvent(e: CustomEvent<DndEvent<Hunk>>) {
		hunks = e.detail.items;
		hunks.sort((itemA, itemB) => compareDesc(itemA.modifiedAt, itemB.modifiedAt));
		if (e.type == 'finalize' && hunks.length == 0) dispatch('empty');
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

	function boldenFilename(filepath: string): string {
		const parts = filepath.split('/');
		if (parts.length == 0) return '';
		return (
			parts.slice(0, -2).join('/') +
			'/<span class="font-bold">' +
			parts[parts.length - 1] +
			'</span>'
		);
	}
</script>

<div
	class="changed-file flex w-full flex-col justify-center gap-2 rounded-lg border border-light-700 bg-white p-2 text-dark-600 dark:border-dark-700 dark:bg-black dark:text-light-300"
>
	<div class="flex items-center gap-2">
		<div class="flex-grow overflow-hidden text-ellipsis whitespace-nowrap" title={filepath}>
			{@html boldenFilename(filepath)}
		</div>
		<div
			on:click={() => (expanded = !expanded)}
			on:keypress={() => (expanded = !expanded)}
			class="cursor-pointer p-2"
		>
			{#if expanded}
				<svg width="9" height="5" viewBox="0 0 9 5" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M1.31965 5H7.51628C8.26728 5 8.68796 4.24649 8.22398 3.7324L5.12566 0.299447C4.76532 -0.0998156 4.07062 -0.0998156 3.71027 0.299447L0.611959 3.7324C0.147977 4.24649 0.568658 5 1.31965 5Z"
						fill="currentColor"
					/>
				</svg>
			{:else}
				<svg width="9" height="5" viewBox="0 0 9 5" fill="none" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M7.51628 3.98009e-07L1.31965 -1.43717e-07C0.568658 -2.09371e-07 0.147978 0.75351 0.611959 1.2676L3.71027 4.70055C4.07062 5.09982 4.76532 5.09982 5.12566 4.70055L8.22398 1.2676C8.68796 0.753511 8.26728 4.63664e-07 7.51628 3.98009e-07Z"
						fill="currentColor"
					/>
				</svg>
			{/if}
		</div>
	</div>

	<div
		class="hunk-change-container flex flex-col gap-2 rounded"
		bind:this={zoneEl}
		use:dndzone={{
			items: hunks,
			zoneTabIndex: -1,
			autoAriaDisabled: true,
			types: ['hunk', filepath],
			receives: [filepath]
		}}
		on:consider={handleDndEvent}
		on:finalize={handleDndEvent}
	>
		{#if expanded}
			{#each hunks || [] as hunk (hunk.id)}
				<div
					class="changed-hunk flex w-full flex-col gap-1 rounded-sm border border-light-500 dark:border-dark-500"
				>
					<div class="w-full text-ellipsis p-2">
						{hunk.name}
					</div>
					<div
						class="cursor-pointer border-t border-b border-light-700 text-sm dark:border-dark-800"
					>
						<Differ
							diff={diffStringToDiffArray(hunk.diff)}
							lineNumberOffset={diffLineNumberOffset(hunk.diff)}
							filepath={hunk.filePath}
						/>
					</div>
					<div class="flex p-2 text-sm">
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
