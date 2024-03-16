<script lang="ts">
	import CommitListFooter from './CommitListFooter.svelte';
	import CommitListHeader from './CommitListHeader.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import type { Project } from '$lib/backend/projects';
	import type {
		AnyFile,
		BaseBranch,
		Branch,
		Commit,
		CommitStatus,
		RemoteCommit
	} from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let project: Project;
	export let type: CommitStatus;
	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;
	export let branchCount: number = 0;
	export let commits: Commit[] | RemoteCommit[];

	let headerHeight: number;

	$: headCommit = branch.commits[0];

	$: hasCommits = commits && commits.length > 0;
	$: remoteRequiresForcePush = type === 'remote' && branch.requiresForce;

	let expanded = true;
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
								{branch}
								{commit}
								{base}
								{project}
								{isUnapplied}
								{selectedFiles}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						{/each}
					</div>
				{/if}
				{#if type == 'upstream' && branchCount > 1}
					<div class="upstream-message text-base-body-11">
						You have {branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>{/if}
				<CommitListFooter
					{branch}
					{type}
					{base}
					{isUnapplied}
					projectId={project.id}
					{hasCommits}
				/>
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
		padding: 0 var(--space-14) var(--space-14) var(--space-14);
		gap: var(--space-8);
	}
	.upstream-message {
		color: var(--clr-theme-scale-warn-30);
		border-radius: var(--radius-m);
		background: var(--clr-theme-scale-warn-80);
		padding: var(--space-12);
		margin-left: var(--space-16);
	}

	.commits {
		display: flex;
		flex-direction: column;
	}
</style>
