<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import VirtualList from '$components/v3/VirtualList.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { onMount } from 'svelte';
	import type { Commit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [uiState, baseBranchService, stackService] = inject(
		UiState,
		BaseBranchService,
		StackService
	);

	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);
	const baseBranchResult = $derived(baseBranchService.baseBranch(projectId));
	const baseSha = $derived(baseBranchResult.current.data?.baseSha);

	let loadedIds = $state<string[]>([]);
	let commits = $state<Commit[]>([]);
	let loading = $state(false);
	let throttled = $state(false);

	async function loadMore() {
		if (loading) {
			throttled = true;
			return;
		}
		loading = true;
		try {
			if (!baseSha) return;
			if (loadedIds.length === 0) {
				commits = commits.concat(await getPage(baseSha));
				loadedIds.push(baseSha);
				return;
			}
			const nextId = commits.at(-1)?.id;
			if (nextId && !loadedIds.includes(nextId)) {
				commits = commits.concat(await getPage(nextId));
				loadedIds.push(nextId);
			}
			if (throttled) {
				loadMore();
			}
		} finally {
			loading = false;
		}
	}

	async function getPage(commitId: string) {
		const result = await stackService.targetCommits(projectId, commitId, 20);
		return result.data || [];
	}

	onMount(() => {
		loadMore();
	});
</script>

<ReduxResult {projectId} result={baseBranchResult.current}>
	{#snippet children(baseBranch)}
		{@const lastUpdate = baseBranch.recentCommits.at(0)?.createdAt.getTime() || 0}

		<div class="target-branch-header">
			<BranchHeaderIcon
				lineColor={getColorFromBranchType('LocalAndRemote')}
				iconName="home"
				lineTop={false}
				lineBottom
			/>
			<div class="target-branch-header__content">
				<h3 class="text-15 text-bold truncate">{baseBranch.branchName}</h3>

				<div class="target-branch-header__content-details">
					<p class="text-12 target-branch-header__caption truncate">Current workspace target</p>
					<Tooltip text="Last update {new Date(lastUpdate).toLocaleString()}">
						<p class="text-12 target-branch-header__caption">
							{getTimeAgo(new Date(lastUpdate))}
						</p>
					</Tooltip>
				</div>
			</div>
		</div>
		<VirtualList items={commits} batchSize={10} onloadmore={async () => await loadMore()}>
			{#snippet group(commits)}
				{#each commits as commit}
					{@const selected = commit.id === branchesState?.current.commitId}
					<CommitRow
						disableCommitActions
						type="LocalAndRemote"
						{projectId}
						{selected}
						commitId={commit.id}
						branchName={baseBranch.branchName}
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						onclick={() => {
							branchesState.set({
								commitId: commit.id,
								branchName: baseBranch.shortName,
								remote: baseBranch.remoteName
							});
						}}
					/>
				{/each}
			{/snippet}
		</VirtualList>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.target-branch-header {
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;
	}

	.target-branch-header__content {
		display: flex;
		flex-direction: column;
		flex: 1;
		gap: 5px;
		padding: 12px 12px 12px 4px;
		overflow: hidden;
	}

	.target-branch-header__content-details {
		display: flex;
		justify-content: space-between;
		gap: 6px;
	}

	.target-branch-header__caption {
		color: var(--clr-text-2);

		white-space: nowrap;
	}
</style>
