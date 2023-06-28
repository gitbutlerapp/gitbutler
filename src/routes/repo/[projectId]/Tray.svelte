<script lang="ts">
	import { Button, Checkbox } from '$lib/components';
	import type { Branch, BranchData, Target } from './types';
	import { formatDistanceToNow } from 'date-fns';
	import type { VirtualBranchOperations } from './vbranches';

	export let target: Target;
	export let branches: Branch[];
	export let remoteBranches: BranchData[];
	export let virtualBranches: VirtualBranchOperations;

	// store left tray width preference in localStorage
	const cacheKey = 'config:tray-width';

	function rememberWidth(node: HTMLElement) {
		const cachedWidth = localStorage.getItem(cacheKey);
		if (cachedWidth) node.style.width = cachedWidth;

		const resizeObserver = new ResizeObserver((entries) => {
			const width = entries.at(0)?.borderBoxSize[0].inlineSize.toString();
			if (width) localStorage.setItem(cacheKey, width + 'px');
		});
		resizeObserver.observe(node);

		return {
			destroy: () => {
				resizeObserver.unobserve(node);
			}
		};
	}
</script>

<div
	use:rememberWidth
	class="w-80 shrink-0 resize-x overflow-x-auto overflow-y-auto bg-light-100 px-2 text-light-800 dark:bg-dark-800 dark:text-dark-100"
>
	<div class="py-4 text-lg font-bold">Your target</div>
	<div class="flex flex-col gap-y-2">
		<div>{target.name}</div>
		{#if target.behind > 0}
			<div class="flex flex-row justify-between">
				<div>behind {target.behind}</div>
				<Button on:click={virtualBranches.updateBranchTarget}>Update Target</Button>
			</div>
		{:else}
			<div class="flex flex-row justify-between">
				<div>up to date</div>
			</div>
		{/if}
	</div>

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
						<div title={branch.authors.join('\n')}>
							{#each branch.authors as author}
								{author[0]}
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/each}
	{/if}
</div>
