<script lang="ts">
	import { Checkbox } from '$lib/components';
	import type { Branch } from './types';

	export let branches: Branch[];
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
</div>
