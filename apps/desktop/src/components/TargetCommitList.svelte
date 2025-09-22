<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import VirtualList from '$components/VirtualList.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';

	import { TestId, TimeAgo } from '@gitbutler/ui';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { onMount } from 'svelte';
	import type { Commit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const uiState = inject(UI_STATE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseSha = $derived(baseBranchQuery.response?.baseSha);

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
				commits = commits.concat(await getPage(undefined));
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

	async function getPage(commitId: string | undefined) {
		const result = await stackService.targetCommits(projectId, commitId, 20);
		return result || [];
	}

	onMount(() => {
		loadMore();
	});
</script>

<ReduxResult {projectId} result={baseBranchQuery.result}>
	{#snippet children(baseBranch)}
		{@const lastUpdate = baseBranch.recentCommits.at(0)?.createdAt.getTime() || 0}

		<div data-testid={TestId.TargetCommitListHeader} class="target-branch-header">
			<div class="target-branch-header__content">
				<div class="flex gap-8">
					<BranchHeaderIcon
						color={getColorFromBranchType('LocalAndRemote')}
						iconName="home-small"
					/>
					<h3 class="text-15 text-bold truncate">{baseBranch.branchName}</h3>
				</div>

				<div class="target-branch-header__content-details">
					<p class="text-12 target-branch-header__caption truncate">Current workspace target</p>
					<p class="text-12 target-branch-header__caption">
						<TimeAgo date={new Date(lastUpdate)} />
					</p>
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
						diverged={commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject}
						{selected}
						commitId={commit.id}
						branchName={baseBranch.branchName}
						commitMessage={commit.message}
						createdAt={commit.createdAt}
						author={commit.author}
						editable={false}
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
		padding: 14px;
		border-bottom: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		background-color: var(--clr-bg-2);
	}

	.target-branch-header__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		gap: 8px;
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
