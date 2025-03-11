<script lang="ts">
	import Badge from '@gitbutler/ui/Badge.svelte';

	type Props = {
		commitId: string;
		first: boolean;
		last: boolean;
		selected: boolean;
		onclick: () => void;
	};

	const { commitId, first, last, selected, onclick }: Props = $props();
</script>

{#snippet indicator(args?: { last: boolean; first: boolean })}
	<div class="indicator" class:first={args?.first} class:last={args?.last}>
		<div class="pin">
			<div class="pin__line"></div>
			<div class="pin__circle"></div>
		</div>
		<div>
			<Badge size="tag" style="pop">Your commit goes here {last}</Badge>
		</div>
	</div>
{/snippet}
{#snippet commitHere(args: { commitId: string; last?: boolean })}
	<button class="commit-here" type="button" class:last={args.last} {onclick}>
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

{#if selected}
	{@render indicator({ first, last })}
{/if}
{#if !selected}
	{@render commitHere({ commitId })}
{/if}

<style lang="postcss">
	.indicator {
		padding: 12px 0;
		display: flex;
		gap: 12px;
		align-items: center;
		background-color: var(--clr-bg-1);
		border-bottom: 1px solid var(--clr-border-2);
		&.first {
			border-top: 1px solid var(--clr-border-2);
		}
		&.last {
			border-top: 1px solid var(--clr-border-2);
			border-bottom: none;
			border-radius: 0 0 var(--radius-l) var(--radius-l);
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

	/* COMMIT HERE */
	.commit-here {
		width: 100%;
		position: relative;
		height: 20px;
		margin-top: -10px;
		margin-bottom: -10px;
		display: flex;
		align-items: center;
		opacity: 0;
		z-index: var(--z-lifted);
		&:hover {
			opacity: 1;
		}
		&.last {
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
