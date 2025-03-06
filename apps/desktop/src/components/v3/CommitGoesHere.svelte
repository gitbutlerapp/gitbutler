<script lang="ts">
	import BranchCommitList from './BranchCommitList.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import CommitRow from './CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { createCommitPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
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

{#snippet indicator(args?: { last?: boolean; first?: boolean })}
	<div class="indicator" class:first={args?.first} class:last={args?.last}>
		<div class="pin">
			<div class="pin__line"></div>
			<div class="pin__circle"></div>
		</div>
		<div>
			<Badge size="tag" style="pop">Your commit goes here</Badge>
		</div>
	</div>
{/snippet}
{#snippet commitHere(args: { commitId: string; last?: boolean })}
	<button
		class="commit-here"
		type="button"
		class:last={args.last}
		onclick={() => goto(createCommitPath(projectId, stackId, branchName, args.commitId))}
	>
		<div class="commit-here__circle"></div>
		<div class="commit-here__line"></div>
		<div class="commit-here__label text-11 text-semibold">Commit here</div>
	</button>
{/snippet}
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
							isTopBranch={i === 0}
							lineColor="var(--clr-commit-local)"
							readonly
						/>
					</div>
					<BranchCommitList
						{projectId}
						{stackId}
						branchName={branch.name}
						selectedCommitId={parentId}
					>
						{#snippet localAndRemoteTemplate({
							commit,
							commitKey,
							first,
							lastCommit: last,
							selected
						})}
							{@const baseSha = $baseBranch?.baseSha}
							{#if selected}
								{@render indicator({ first })}
							{/if}
							<div class="commit-wrapper" class:last>
								{#if !selected}
									{@render commitHere({ commitId: commit.id })}
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
									href={createCommitPath(projectId, stackId, branchName, commit.id)}
								/>
								{#if lastBranch && last && baseSha && parentId !== baseSha}
									{@render commitHere({ commitId: baseSha, last: true })}
								{/if}
							</div>
							{#if lastBranch && last && parentId === baseSha}
								{@render indicator({ last: true })}
							{/if}
						{/snippet}
					</BranchCommitList>
				</div>
			{/each}
		{/snippet}
	</ReduxResult>
</div>

<style lang="postcss">
	.commit-goes-here {
		display: flex;
		flex-direction: column;
	}

	.branch {
		display: flex;
		flex-direction: column;
		margin-bottom: 14px;
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
	.indicator {
		padding: 12px 0;
		display: flex;
		gap: 12px;
		align-items: center;
		background-color: var(--clr-bg-1);
		&.first,
		&.last {
			border-top: 1px solid var(--clr-border-2);
		}
		&.last {
			border-bottom: none;
		}
	}
	.pin {
		display: flex;
		align-items: center;
		width: 40px;
		height: 10px;
		margin-left: -15px;
		position: relative;
	}
	.pin__line {
		flex-grow: 1;
		height: 2px;
		background-color: var(--clr-theme-pop-element);
	}
	.pin__circle {
		border-radius: 100%;
		width: 10px;
		height: 10px;
		outline: 2px solid var(--clr-theme-pop-element);
	}
	.commit-wrapper {
		position: relative;
		display: flex;
		width: 100%;
		background-color: var(--clr-bg-2);
		&.last {
			border-radius: 0 0 var(--radius-l) var(--radius-l);
		}
	}

	/* COMMIT HERE */
	.commit-here {
		width: 100%;
		position: absolute;
		height: 100%;
		top: -50%;
		display: flex;
		align-items: center;
		opacity: 0;
		z-index: var(--z-lifted);
		&:hover {
			opacity: 1;
		}
		&.last {
			bottom: -50%;
			top: unset;
		}
	}
	.commit-here__circle {
		position: absolute;
		left: 16px;
		top: 50%;
		transform: translateY(-50%);
		border-radius: 100%;
		width: 10px;
		height: 10px;
		background-color: var(--clr-theme-pop-element);
		outline: 2px solid var(--clr-bg-2);
	}
	.commit-here__line {
		background-color: var(--clr-theme-pop-element);
		height: 2px;
		flex-grow: 1;
		margin-left: -15px;
	}
	.commit-here__label {
		position: absolute;
		top: 50%;
		left: 38px;
		transform: translateY(-50%);
		padding: 2px 6px;
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-core-ntrl-100);
	}
</style>
