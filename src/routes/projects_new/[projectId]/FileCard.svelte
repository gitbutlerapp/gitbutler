<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { File } from './board';
	const flipDurationMs = 150;
	import animateHeight from './animation';

	export let file: File;
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
		use:dndzone={{ items: file.hunks, flipDurationMs, zoneTabIndex: -1, type: file.path }}
		on:consider={(e) => (file.hunks = e.detail.items)}
		on:finalize={(e) => (file.hunks = e.detail.items)}
	>
		{#each file.hunks as hunk (hunk.id)}
			<div
				animate:flip={{ duration: flipDurationMs }}
				class="w-full rounded border border-zinc-500 bg-zinc-600 p-1"
			>
				{hunk.name}
			</div>
		{/each}
	</div>
</div>
