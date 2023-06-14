<script lang="ts">
	import { flip } from 'svelte/animate';
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';

	export let branches: Branch[];
</script>

<section class="flex h-full w-64 flex-col gap-y-2 border-r border-zinc-700 bg-[#2F2F33] p-4">
	<div class="flex flex-col gap-y-2 py-2">
		{#each branches as branch (branch.id)}
			<div animate:flip={{ duration: 300 }} class="rounded border border-zinc-600 bg-zinc-700 p-2">
				<Checkbox bind:checked={branch.active} />
				<span class="ml-2">{branch.name}</span>
			</div>
		{/each}
	</div>
</section>
