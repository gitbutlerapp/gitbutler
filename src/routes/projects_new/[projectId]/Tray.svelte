<script lang="ts">
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';

	export let branches: Branch[];
</script>

<div class="w-80 shrink-0 bg-dark-300 px-2 text-light-800">
	<div class="py-4">Your Branches</div>
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
</div>
