<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { File, Hunk } from './board';
	const flipDurationMs = 150;
	import animateHeight from './animation';
	import { formatDistanceToNow, compareDesc } from 'date-fns';

	export let file: File;

	function sortAndUpdateHunks(e: { detail: { items: Hunk[] } }) {
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
			type: file.path,
			autoAriaDisabled: true
		}}
		on:consider={sortAndUpdateHunks}
		on:finalize={sortAndUpdateHunks}
	>
		{#each file.hunks as hunk (hunk.id)}
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
