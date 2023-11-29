<script lang="ts">
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';
	import CommitListItem from './CommitListItem.svelte';
	import type { PrService } from '$lib/github/pullrequest';
	import CommitListHeader from './CommitListHeader.svelte';
	import type { CommitType } from './commitList';
	import CommitListFooter from './CommitListFooter.svelte';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;
	export let branchController: BranchController;
	export let type: CommitType;
	export let prService: PrService;
	export let readonly: boolean;

	export let acceptAmend: (commit: Commit) => (data: any) => boolean;
	export let acceptSquash: (commit: Commit) => (data: any) => boolean;
	export let onAmend: (data: DraggableFile | DraggableHunk) => void;
	export let onSquash: (commit: Commit) => (data: DraggableCommit) => void;
	export let resetHeadCommit: () => void;

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
	$: pr$ = prService.get(branch.shortName);

	let expanded = true;
</script>

{#if commits.length > 0}
	<div class="wrapper">
		<CommitListHeader {expanded} {branch} {pr$} {type} {base} />
		{#if expanded}
			<div class="content-wrapper">
				<div class="commits">
					{#each commits as commit, idx (commit.id)}
						<div class="draggable-wrapper">
							<CommitListItem
								{commit}
								{base}
								{projectId}
								{readonly}
								{acceptAmend}
								{acceptSquash}
								{onAmend}
								{onSquash}
								{resetHeadCommit}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						</div>
					{/each}
				</div>
			</div>
			<CommitListFooter
				{branchController}
				{branch}
				{prService}
				{type}
				{base}
				{githubContext}
				{readonly}
				{projectId}
			/>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
	.content-wrapper {
		display: flex;
		flex-direction: column;
		padding: 0 var(--space-16) var(--space-20) var(--space-16);
		gap: var(--space-8);
	}
	.commits {
	}
</style>
