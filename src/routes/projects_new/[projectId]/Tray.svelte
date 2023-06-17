<script lang="ts">
	import { flip } from 'svelte/animate';
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';

	export let branches: Branch[];
</script>

<section class="flex h-full w-64 flex-col gap-y-2 border-r border-zinc-700 bg-[#2C2C2C] p-4">
	<div class="flex flex-col gap-y-2 py-2">
		{#each branches as branch (branch.id)}
			<div
				class="rounded border-t-[0.5px] border-b-[0.5px] border-t-[#4C4C4C] border-b-[#252525] bg-[#212121] p-2"
			>
				<Checkbox bind:checked={branch.active} />
				<span class="ml-2">{branch.name}</span>
			</div>
		{/each}
	</div>
</section>
