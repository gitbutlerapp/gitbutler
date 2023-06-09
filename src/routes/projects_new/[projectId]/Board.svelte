<script lang="ts">
	import { flip } from 'svelte/animate';
	import { dndzone } from 'svelte-dnd-action';
	import type { BranchLane } from './board';
	import Lane from './Lane.svelte';

	export let columns: BranchLane[];

	const flipDurationMs = 300;
</script>

<section
	class="flex gap-x-4 p-4"
	use:dndzone={{ items: columns, flipDurationMs, type: 'column' }}
	on:consider={(e) => (columns = e.detail.items)}
	on:finalize={(e) => (columns = e.detail.items)}
>
	{#each columns.filter((c) => c.active) as { id, name, files }, idx (id)}
		<div
			class="flex w-64 border border-zinc-700 bg-zinc-900/50 p-4"
			animate:flip={{ duration: flipDurationMs }}
		>
			<Lane {name} bind:files />
		</div>
	{/each}
</section>
