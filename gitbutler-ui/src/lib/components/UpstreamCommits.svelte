<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import Button from '$lib/components/Button.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableRemoteCommit } from '$lib/dragging/draggables';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, LocalFile, RemoteBranchData } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branchId: string;
	export let projectId: string;
	export let branchCount: number;
	export let upstream: RemoteBranchData | undefined;
	export let branchController: BranchController;
	export let base: BaseBranch | undefined | null;
	export let selectedFiles: Writable<LocalFile[]>;

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
				kind="outlined"
				color="primary"
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
					<CommitCard
						{commit}
						{projectId}
						commitUrl={base?.commitUrl(commit.id)}
						{branchController}
						{selectedFiles}
					/>
				</div>
			{/each}
			<div class="flex justify-end p-2">
				{#if branchCount > 1}
					<div class="px-2 text-sm">
						You have {branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>
				{/if}
				<Button color="primary" on:click={merge}>Merge</Button>
			</div>
		</div>
	{/if}
{/if}
