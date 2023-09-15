<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, RemoteBranch } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let branch: RemoteBranch | undefined;
	export let base: BaseBranch | undefined;
	export let branchController: BranchController;
</script>

{#if branch != undefined}
	<div class="flex w-full max-w-full flex-col gap-y-4 p-4">
		<div class="flex-grow items-center">
			<p class="truncate text-lg font-bold" title="remote branch">
				{branch.name.replace('refs/remotes/', '').replace('origin/', '').replace('refs/heads/', '')}
			</p>
			<p class="text-3" title="upstream target">
				{branch.upstream?.replace('refs/remotes/', '') || ''}
			</p>
		</div>
		<Button
			color="purple"
			height="small"
			on:click={() => branch && branchController.createvBranchFromBranch(branch.name)}
		>
			Apply
		</Button>
		{#if branch.commits && branch.commits.length > 0}
			<div class="flex w-full flex-col gap-y-2">
				{#each branch.commits as commit}
					<CommitCard
						{commit}
						url={base?.commitUrl(commit.id)}
						projectId={branchController.projectId}
					/>
				{/each}
			</div>
		{/if}
	</div>
{/if}
