<script lang="ts">
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import CommitListItem from './CommitListItem.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import CommitListHeader from './CommitListHeader.svelte';
	import type { CommitType } from './commitList';
	import CommitListFooter from './CommitListFooter.svelte';
	import type { Project } from '$lib/backend/projects';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let project: Project;
	export let branchController: BranchController;
	export let type: CommitType;
	export let prService: PrService;
	export let readonly: boolean;

	let headerHeight: number;

	$: headCommit = branch.commits[0];
	$: commits = branch.commits.filter((c) => {
		switch (type) {
			case 'local':
				return !c.isIntegrated && !c.isRemote;
			case 'remote':
				return !c.isIntegrated && c.isRemote;
			case 'integrated':
				return c.isIntegrated;
		}
	});
	$: pr$ = prService.get(branch.upstreamName);
	// $: prStatus$ = prService.getStatus($pr$?.targetBranch);

	let expanded = true;
</script>

{#if commits.length > 0}
	<div class="commit-list" style:min-height={expanded ? `${2 * headerHeight}px` : undefined}>
		<CommitListHeader bind:expanded {branch} {pr$} {type} {base} bind:height={headerHeight} />
		{#if expanded}
			<div class="commit-list__content">
				<div class="commits">
					{#each commits as commit, idx (commit.id)}
						<CommitListItem
							{branch}
							{branchController}
							{commit}
							{base}
							{project}
							{readonly}
							isChained={idx != commits.length - 1}
							isHeadCommit={commit.id === headCommit?.id}
						/>
					{/each}
				</div>
				<CommitListFooter
					{branchController}
					{branch}
					{prService}
					{type}
					{base}
					{githubContext}
					{readonly}
					projectId={project.id}
				/>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.commit-list {
		background-color: var(--clr-theme-container-light);
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-theme-container-outline-light);
		position: relative;
		flex-shrink: 0;
	}
	.commit-list__content {
		display: flex;
		flex-direction: column;
		padding: 0 var(--space-16) var(--space-20) var(--space-16);
		gap: var(--space-8);
	}
</style>
