<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import VirtualList from '$components/v3/VirtualList.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { onMount } from 'svelte';
	import type { Commit } from '$lib/branches/v3';

	type Props = {
		projectId: string;
		branchName: string;
	};

	const { projectId, branchName }: Props = $props();

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
		<BranchCard type="normal-branch" {projectId} {branchName} expand>
			{#snippet header()}
				<BranchHeader
					type="normal-branch"
					{branchName}
					{projectId}
					selected
					lineColor={getColorFromBranchType('LocalOnly')}
					iconName="branch-upstream"
					lastUpdatedAt={baseBranch.recentCommits.at(0)?.createdAt.getTime()}
					isTopBranch
					readonly
					onclick={() => {
						uiState.project(projectId).branchesSelection.set({
							branchName
						});
					}}
				/>
			{/snippet}
			{#snippet commitList()}
				<VirtualList items={commits} batchSize={10} onloadmore={async () => await loadMore()}>
					{#snippet group(commits)}
						{#each commits as commit}
							{@const selected = commit.id === branchesState?.current.commitId}
							<CommitRow
								disableCommitActions
								type="Remote"
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
		</BranchCard>
	{/snippet}
</ReduxResult>

<style lang="postcss">
</style>
