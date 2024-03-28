<script lang="ts">
	import CommitListFooter from './CommitListFooter.svelte';
	import CommitListHeader from './CommitListHeader.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { VirtualBranchService } from '$lib/vbranches/branchStoresCache';
	import {
		Branch,
		type AnyFile,
		type Commit,
		type CommitStatus,
		type RemoteCommit
	} from '$lib/vbranches/types';
	import { map } from 'rxjs';
	import type { Writable } from 'svelte/store';

	export let type: CommitStatus;
	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;
	export let commits: Commit[] | RemoteCommit[];

	const branchService = getContext(VirtualBranchService);
	const branch = getContextStore(Branch);

	let headerHeight: number;
	let expanded = true;

	$: headCommit = $branch.commits[0];
	$: hasCommits = commits && commits.length > 0;

	$: remoteRequiresForcePush = type === 'remote' && $branch.requiresForce;
	$: branchCount = branchService.activeBranches$.pipe(map((branches) => branches?.length || 0));
</script>

{#if hasCommits || remoteRequiresForcePush}
	<div class="commit-list card" class:upstream={type == 'upstream'}>
		<CommitListHeader
			{type}
			bind:expanded
			bind:height={headerHeight}
			isExpandable={hasCommits}
			commitCount={commits.length}
		/>
		{#if expanded}
			<div class="commit-list__content">
				{#if hasCommits}
					<div class="commits">
						{#each commits as commit, idx (commit.id)}
							<CommitListItem
								{commit}
								{isUnapplied}
								{selectedFiles}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						{/each}
					</div>
				{/if}
				{#if type == 'upstream' && $branchCount > 1}
					<div class="upstream-message text-base-body-11">
						You have {$branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>{/if}
				<CommitListFooter {type} {isUnapplied} {hasCommits} />
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.commit-list {
		&.upstream {
			background-color: var(--clr-theme-container-pale);
		}
		background-color: var(--clr-theme-container-light);
		display: flex;
		flex-direction: column;
		position: relative;
		flex-shrink: 0;
	}
	.commit-list__content {
		display: flex;
		flex-direction: column;
		padding: 0 var(--size-14) var(--size-14) var(--size-14);
		gap: var(--size-8);
	}
	.upstream-message {
		color: var(--clr-theme-scale-warn-30);
		border-radius: var(--radius-m);
		background: var(--clr-theme-scale-warn-80);
		padding: var(--size-12);
		margin-left: var(--size-16);
	}

	.commits {
		display: flex;
		flex-direction: column;
	}
</style>
