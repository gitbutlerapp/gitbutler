<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { File } from './board';

	export let name: string;
	export let files: File[];

	const flipDurationMs = 150;
</script>

<div class="flex h-full w-full flex-col">
	<div class="flex items-center justify-center p-4 font-bold">
		{name}
	</div>
	<div
		class="flex flex-grow flex-col gap-y-4"
		use:dndzone={{ items: files, flipDurationMs, zoneTabIndex: -1 }}
		on:consider={(e) => (files = e.detail.items)}
		on:finalize={(e) => (files = e.detail.items)}
	>
		{#each files as file (file.id)}
			<div
				animate:flip={{ duration: flipDurationMs }}
				class="flex h-14 w-full items-center justify-center rounded border border-zinc-600 bg-zinc-700"
			>
				{file.path}
			</div>
		{/each}
	</div>
</div>
