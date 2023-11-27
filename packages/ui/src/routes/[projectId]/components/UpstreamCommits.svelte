<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import { draggableRemoteCommit } from '$lib/draggables';
	import { draggable } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, RemoteBranch } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let branchId: string;
	export let projectId: string;
	export let branchCount: number;
	export let upstream: RemoteBranch | undefined;
	export let branchController: BranchController;
	export let base: BaseBranch | undefined | null;

	let upstreamCommitsShown = false;

	$: if (upstreamCommitsShown && upstream?.commits.length === 0) {
		upstreamCommitsShown = false;
	}

	function merge() {
		branchController.mergeUpstream(branchId);
	}
</script>

{#if upstream}
	<div class="bg-zinc-300 p-2 dark:bg-zinc-800">
		<div class="flex flex-row justify-between">
			<div class="p-1 text-purple-700">
				{upstream.commits.length}
				upstream {upstream.commits.length > 1 ? 'commits' : 'commit'}
			</div>
			<Button
				class="w-20"
				height="small"
				kind="outlined"
				color="purple"
				on:click={() => (upstreamCommitsShown = !upstreamCommitsShown)}
			>
				<span class="purple">
					{#if !upstreamCommitsShown}
						View
					{:else}
						Cancel
					{/if}
				</span>
			</Button>
		</div>
	</div>
	{#if upstreamCommitsShown}
		<div
			class="flex w-full flex-col gap-1 border-t border-light-400 bg-light-300 p-2 dark:border-dark-400 dark:bg-dark-800"
			id="upstreamCommits"
		>
			{#each upstream.commits as commit (commit.id)}
				<div use:draggable={draggableRemoteCommit(branchId, commit)}>
					<CommitCard {commit} {projectId} commitUrl={base?.commitUrl(commit.id)} />
				</div>
			{/each}
			<div class="flex justify-end p-2">
				{#if branchCount > 1}
					<div class="px-2 text-sm">
						You have {branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>
				{/if}
				<Button class="w-20" height="small" color="purple" on:click={merge}>Merge</Button>
			</div>
		</div>
	{/if}
{/if}
