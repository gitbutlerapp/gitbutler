<script lang="ts">
	import CommitListFooter from './CommitListFooter.svelte';
	import CommitListHeader from './CommitListHeader.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import type { Project } from '$lib/backend/projects';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { AnyFile, BaseBranch, Branch, CommitStatus } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branch: Branch;
	export let base: BaseBranch | undefined | null;
	export let project: Project;
	export let branchController: BranchController;
	export let type: CommitStatus;
	export let githubService: GitHubService;
	export let branchService: BranchService;
	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;
	export let branchCount: number = 0;

	let headerHeight: number;

	$: headCommit = branch.commits[0];

	$: commits = type == 'upstream' ? [] : branch.commits.filter((c) => c.status == type);
	$: hasCommits = commits && commits.length > 0;
	$: remoteRequiresForcePush = type === 'remote' && branch.requiresForce;

	let expanded = true;
</script>

{#if hasCommits || remoteRequiresForcePush}
	<div
		class="commit-list card"
		class:upstream={type == 'upstream'}
		style:min-height={expanded ? `${2 * headerHeight}px` : undefined}
	>
		<CommitListHeader {type} bind:expanded bind:height={headerHeight} />
		{#if expanded}
			<div class="commit-list__content">
				<div class="commits">
					{#if commits}
						{#each commits as commit, idx (commit.id)}
							<CommitListItem
								{branch}
								{branchController}
								{commit}
								{base}
								{project}
								{isUnapplied}
								{selectedFiles}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						{/each}
					{/if}
				</div>
				{#if type == 'upstream' && branchCount > 1}
					<div class="upstream-message text-base-body-11">
						You have {branchCount} active branches. To merge upstream work, we will unapply all other
						branches.
					</div>{/if}
				<CommitListFooter
					{branchController}
					{branchService}
					{branch}
					{githubService}
					{type}
					{base}
					{isUnapplied}
					projectId={project.id}
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
		padding: 0 var(--space-16) var(--space-20) var(--space-16);
		gap: var(--space-8);
	}
	.upstream-message {
		color: var(--clr-theme-scale-warn-30);
		border-radius: var(--radius-m);
		background: var(--clr-theme-scale-warn-80);
		padding: var(--space-12);
		margin-left: var(--space-16);
	}
</style>
