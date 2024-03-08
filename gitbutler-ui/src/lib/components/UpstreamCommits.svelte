<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import Button from '$lib/components/Button.svelte';
	import { draggable } from '$lib/dragging/draggable';
	import { draggableRemoteCommit } from '$lib/dragging/draggables';
	import type { Project } from '$lib/backend/projects';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, LocalFile, RemoteBranchData } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project | undefined;
	export let branchId: string;
	export let projectId: string;
	export let branchCount: number;
	export let upstream: RemoteBranchData | undefined;
	export let branchController: BranchController;
	export let base: BaseBranch | undefined | null;
	export let selectedFiles: Writable<LocalFile[]>;

	function merge() {
		branchController.mergeUpstream(branchId);
	}
</script>

{#if upstream}
	<div class="card">
		<div class="card__header text-semibold text-base-12">
			Upstream {upstream.commits.length > 1 ? 'commits' : 'commit'}
		</div>
		<div class="card__content" id="upstreamCommits">
			{#each upstream.commits as commit (commit.id)}
				<div use:draggable={draggableRemoteCommit(branchId, commit)}>
					<CommitCard
						{commit}
						{project}
						{projectId}
						commitUrl={base?.commitUrl(commit.id)}
						{branchController}
						{selectedFiles}
					/>
				</div>
			{/each}
			{#if branchCount > 1}
				<div class="flex justify-end p-2">
					<div class="px-2 text-sm">
						You have {branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>
				</div>
			{/if}
		</div>
		<div class="card__footer">
			<Button wide color="primary" on:click={merge}>Merge</Button>
		</div>
	</div>
{/if}
