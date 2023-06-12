<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import type { Commit, File, Hunk } from './types';
	import CommitGroup from './CommitGroup.svelte';

	export let name: string;
	export let commits: Commit[];

	const flipDurationMs = 150;

	function handleDndEvent(
		e: CustomEvent<DndEvent<Commit | File | Hunk>> & { target: HTMLElement }
	) {
		commits = e.detail.items.filter((item) => item.kind == 'commit') as Commit[];
		// TODO: Create lanes out of dropped files/hunks
	}

	function handleEmpty() {
		const emptyIndex = commits.findIndex((item) => !item.files || item.files.length == 0);
		if (emptyIndex != -1) {
			commits.splice(emptyIndex, 1);
		}
	}
</script>

<div class="flex h-full w-full flex-col">
	<div class="flex items-center justify-center p-4 font-bold">
		{name}
	</div>
	<div
		class="flex flex-grow flex-col gap-y-4"
		use:dndzone={{
			items: commits,
			flipDurationMs,
			zoneTabIndex: -1,
			types: ['commit'],
			receives: ['commit']
		}}
		on:consider={(e) => handleDndEvent(e)}
		on:finalize={(e) => handleDndEvent(e)}
	>
		{#each commits.filter((x) => x.files) as { id, description, files }, idx (id)}
			<div animate:flip={{ duration: flipDurationMs }}>
				<CommitGroup {id} bind:description bind:files on:empty={handleEmpty} />
			</div>
		{/each}
	</div>
</div>
