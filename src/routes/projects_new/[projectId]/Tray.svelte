<script lang="ts">
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';

	export let branches: Branch[];
	export let remoteBranches: string[];
</script>

<div class="gb-text-2 w-80 shrink-0 px-2">
	<div class="py-4 text-lg font-bold">Your Branches</div>
	<div class="flex flex-col gap-y-2">
		{#each branches as branch (branch.id)}
			<div class="rounded-lg p-2" title={branch.name}>
				<Checkbox bind:checked={branch.active} />
				<span class="ml-2 cursor-pointer">
					{branch.name}
				</span>
			</div>
		{/each}
	</div>
	{#if remoteBranches}
	<div class="flex flex-col gap-y-1">
		<div class="py-4 text-lg font-bold">Remote Branches</div>
		{#each remoteBranches as branch}
			<div class="rounded-lg p-2" title={branch}>
				<span class="ml-2 cursor-pointer">
					{branch}
				</span>
			</div>
		{/each}
	</div>
	{/if}
</div>
