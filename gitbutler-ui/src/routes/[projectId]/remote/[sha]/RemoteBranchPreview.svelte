<script lang="ts">
	import CommitCard from '../../components/CommitCard.svelte';
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { RemoteBranch } from '$lib/vbranches/types';

	export let branch: RemoteBranch | undefined;
	export let projectId: string;
	export let branchController: BranchController;
</script>

{#if branch != undefined}
	<div class="card">
		<div class="card__header text-base-14 text-semibold">
			<span class="card__title" title="remote branch">
				{branch.name.replace('refs/remotes/', '').replace('origin/', '').replace('refs/heads/', '')}
				<span class="text-3" title="upstream target">
					{branch.upstream?.replace('refs/remotes/', '') || ''}
				</span>
			</span>
		</div>
		<div class="card__content">
			{#if branch.commits && branch.commits.length > 0}
				<div class="flex w-full flex-col gap-y-2">
					{#each branch.commits as commit}
						<CommitCard {commit} {projectId} />
					{/each}
				</div>
			{/if}
		</div>
		<div class="card__footer">
			<Button
				color="primary"
				on:click={() => branch && branchController.createvBranchFromBranch(branch.name)}
			>
				Apply
			</Button>
		</div>
	</div>
{/if}
