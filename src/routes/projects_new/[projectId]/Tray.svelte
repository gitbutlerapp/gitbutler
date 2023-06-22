<script lang="ts">
	import { Checkbox } from '$lib/components';
	import type { Branch, BranchData } from './types';
	import { formatDistanceToNow } from 'date-fns';

	export let branches: Branch[];
	export let remoteBranches: BranchData[];
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
		<div class="flex flex-col">
			<div class="py-4 text-lg font-bold">Remote Branches</div>
			{#each remoteBranches as branch}
				<div class="flex flex-col justify-between rounded-lg p-2" title={branch.branch}>
					<div class="flex flex-row justify-between">
						<div class="cursor-pointer">
							{branch.branch.replace('refs/remotes/', '')}
						</div>
						<div>{branch.ahead}/{branch.behind}</div>
					</div>
					{#if branch.lastCommitTs > 0}
						<div class="flex flex-row justify-between">
							<div class="text-sm">{formatDistanceToNow(branch.lastCommitTs * 1000)}</div>
							<div>
								{#each branch.authors as author}
									{author[0]}
								{/each}
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>
