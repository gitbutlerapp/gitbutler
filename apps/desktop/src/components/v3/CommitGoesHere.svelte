<script lang="ts">
	import BranchCommitList from './BranchCommitList.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import CommitRow from './CommitRow.svelte';
	import ScrollableContainer from '../ScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import RowIndicator from '$components/v3/RowIndicator.svelte';
	import RowPlaceholder from '$components/v3/RowPlaceholder.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { createCommitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		parentId: string;
	};

	const { projectId, stackId, branchName, parentId }: Props = $props();
	const [stackService, baseBranchService] = inject(StackService, BaseBranchService);

	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const baseBranch = $derived(baseBranchService.base);
</script>

<ScrollableContainer>
	<div class="commit-goes-here">
		<ReduxResult result={branchesResult.current}>
			{#snippet children(branches)}
				{#each branches as branch, i}
					{@const lastBranch = i === branches.length - 1}
					<div class="branch" class:selected={branch.name === branchName}>
						<div class="header-wrapper">
							<BranchHeader
								{projectId}
								{stackId}
								{branch}
								onclick={() => {
									goto(createCommitPath(projectId, stackId, branch.name), {
										replaceState: true
									});
								}}
								isTopBranch={i === 0}
								lineColor="var(--clr-commit-local)"
								readonly
							/>
						</div>
						<BranchCommitList
							{projectId}
							{stackId}
							branchName={branch.name}
							selectedBranchName={branchName}
							selectedCommitId={parentId}
						>
							{#snippet emptyBranchCommitHere()}
								<RowIndicator first={true} last={true} />
							{/snippet}
							{#snippet localAndRemoteTemplate({ commit, commitKey, first, last, selected })}
								{@const baseSha = $baseBranch?.baseSha}
								{#if selected && branchName === branch.name}
									<RowIndicator {first} />
								{/if}
								<div class="commit-wrapper" class:last>
									{#if !selected}
										<RowPlaceholder
											{stackId}
											{projectId}
											commitId={commit.id}
											branchName={branch.name}
										/>
									{/if}
									<CommitRow
										{projectId}
										{commitKey}
										{first}
										{commit}
										lastCommit={last}
										lineColor="var(--clr-commit-local)"
										opacity={0.4}
										borderTop={selected}
										onclick={() =>
											goto(createCommitPath(projectId, stackId, branch.name, commit.id), {
												replaceState: true
											})}
									/>
									{#if lastBranch && last && baseSha && parentId !== baseSha}
										<RowPlaceholder
											{stackId}
											{projectId}
											commitId={baseSha}
											last={true}
											branchName={branch.name}
										/>
									{/if}
								</div>
								{#if lastBranch && last && parentId === baseSha}
									<RowIndicator last={true} />
								{/if}
							{/snippet}
						</BranchCommitList>
					</div>
				{/each}
			{/snippet}
		</ReduxResult>
	</div>
</ScrollableContainer>

<style lang="postcss">
	.commit-goes-here {
		display: flex;
		flex-direction: column;
		margin-bottom: 14px;
	}

	.branch {
		display: flex;
		flex-direction: column;
		margin: 14px 14px 0 14px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-2);
		&.selected {
			background-color: var(--clr-bg-1);
		}
	}

	.header-wrapper {
		opacity: 0.4;
	}
	.selected .header-wrapper {
		opacity: 1;
	}
	.commit-wrapper {
		position: relative;
		display: flex;
		width: 100%;
		background-color: var(--clr-bg-2);

		/* Last commit row which does not have an "Your commit here" indicator after it */
		&.last:not(:has(~ .indicator)) {
			border-radius: 0 0 var(--radius-l) var(--radius-l);
		}
	}
</style>
