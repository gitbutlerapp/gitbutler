<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import NestedChangedFiles from '$components/NestedChangedFiles.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { commitCreatedAt, type Commit } from '$lib/branches/v3';
	import { createCommitSelection } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	import VirtualList from '@gitbutler/ui/components/VirtualList.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { onMount } from 'svelte';

	type Props = {
		projectId: string;
		onclick: (commitId: string) => void;
		onFileClick: (index: number) => void;
	};

	const { projectId, onclick, onFileClick }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const userSettings = inject(SETTINGS);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseSha = $derived(baseBranchQuery.response?.baseSha);

	let selectedCommitId = $state<string | undefined>();
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
		} finally {
			loading = false;
			if (throttled) {
				throttled = false;
				loadMore();
			}
		}
	}

	async function getPage(commitId: string | undefined) {
		const result = await stackService.targetCommits(projectId, commitId, 50);
		return result || [];
	}

	onMount(() => {
		loadMore();
	});
</script>

<ReduxResult {projectId} result={baseBranchQuery.result}>
	{#snippet children(branch)}
		<BranchCard
			type="normal-branch"
			first
			lineColor={getColorFromBranchType('LocalAndRemote')}
			{projectId}
			branchName={branch.branchName}
			isTopBranch
			iconName="home-small"
			trackingBranch={branch.remoteName || undefined}
			readonly
			selected={false}
			disableClick
			overflowHidden
		>
			{#snippet branchContent()}
				<div class="commit-list">
					<VirtualList
						items={commits}
						defaultHeight={40}
						visibility={$userSettings.scrollbarVisibilityState}
						onloadmore={async () => await loadMore()}
						renderDistance={100}
					>
						{#snippet template(commit, index)}
							<CommitRow
								disableCommitActions
								type="LocalAndRemote"
								diverged={commit.state.type === 'LocalAndRemote' &&
									commit.id !== commit.state.subject}
								commitId={commit.id}
								branchName={branch.branchName}
								commitMessage={commit.message}
								gerritReviewUrl={commit.gerritReviewUrl ?? undefined}
								createdAt={commitCreatedAt(commit)}
								author={commit.author}
								selected={commit.id === selectedCommitId}
								lastCommit={index === commits.length - 1}
								onclick={() => {
									selectedCommitId = commit.id;
									onclick(commit.id);
								}}
							>
								{#snippet changedFiles()}
									{@const changesQuery = stackService.commitChanges(projectId, commit.id)}

									<ReduxResult {projectId} result={changesQuery.result}>
										{#snippet children(changesResult)}
											<NestedChangedFiles
												title="Changed files"
												{projectId}
												draggableFiles
												selectionId={createCommitSelection({ commitId: commit.id })}
												changes={changesResult.changes.filter(
													(change) =>
														!(change.path in (changesResult.conflictEntries?.entries ?? {}))
												)}
												stats={changesResult.stats}
												conflictEntries={changesResult.conflictEntries}
												autoselect
												allowUnselect={false}
												{onFileClick}
											/>
										{/snippet}
									</ReduxResult>
								{/snippet}
							</CommitRow>
						{/snippet}
					</VirtualList>
				</div>
			{/snippet}
		</BranchCard>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.commit-list {
		display: flex;
		position: relative;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--clr-bg-1);
	}
</style>
