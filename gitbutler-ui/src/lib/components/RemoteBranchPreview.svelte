<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import CommitCard from '$lib/components/CommitCard.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { File, RemoteBranch, RemoteFile } from '$lib/vbranches/types';
	import { writable } from 'svelte/store';

	export let branch: RemoteBranch | undefined;
	export let projectId: string;
	export let projectPath: string;
	export let branchController: BranchController;

	const selectedFiles = writable<(File | RemoteFile)[]>([]);
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
						<CommitCard {commit} {projectId} {branchController} {projectPath} {selectedFiles} />
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
