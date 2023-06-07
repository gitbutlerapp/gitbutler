<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { Hunk } from './board';
	import FileCard from './FileCard.svelte';
	const flipDurationMs = 150;

	export let path: string;
	export let hunks: Hunk[];
</script>

<div
	class="w-fulljustify-center flex flex-col gap-2 rounded border border-zinc-600 bg-zinc-700 p-2"
>
	<div class="font-bold text-zinc-200">
		{path}
	</div>

	<div class="flex flex-col items-center gap-1">
		{#each hunks as { id, name }, idx (id)}
			<div class="w-full rounded border border-zinc-500 bg-zinc-600 p-1">
				{name}
			</div>
		{/each}
	</div>
</div>
