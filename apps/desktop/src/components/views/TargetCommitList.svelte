<script lang="ts">
	import BranchCard from "$components/branch/BranchCard.svelte";
	import CommitListItem from "$components/commit/CommitListItem.svelte";
	import ChangedFilesPanel from "$components/files/ChangedFilesPanel.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { commitCommittedAt } from "$lib/branches/v3";
	import { createCommitSelection } from "$lib/selection/key";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { type UpstreamCommit } from "@gitbutler/but-sdk";
	import { inject } from "@gitbutler/core/context";

	import VirtualList from "@gitbutler/ui/components/VirtualList.svelte";
	import { getColorFromBranchType } from "@gitbutler/ui/utils/getColorFromBranchType";
	import { onMount } from "svelte";

	type Props = {
		projectId: string;
		onclick: (commitId: string) => void;
		onFileClick: (index: number) => void;
	};

	const { projectId, onclick, onFileClick }: Props = $props();

	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);

	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));

	let selectedCommitId = $state<string | undefined>();
	let commits = $state<UpstreamCommit[]>([]);
	let hasMore = $state(true);
	let loading = $state(false);
	let throttled = $state(false);

	async function loadMore() {
		if (loading) {
			throttled = true;
			return;
		}
		if (!hasMore) return;
		loading = true;
		try {
			const from = commits.at(-1)?.id;
			const page = await stackService.targetCommits(projectId, from, 50);
			commits = commits.concat(page.commits.map((entry) => entry.commit));
			// The first page ends at the workspace's fork point rather than at the end of
			// history, so only a continuation page can report that nothing older remains.
			hasMore = page.commits.length > 0 && (from === undefined || page.hasMore);
		} finally {
			loading = false;
			if (throttled) {
				throttled = false;
				loadMore();
			}
		}
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
			lineColor={getColorFromBranchType("LocalAndRemote")}
			{projectId}
			branchName={branch.branchName}
			isTopBranch
			iconName="home"
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
						visibility={uiState.global.scrollbarVisibilityState.current}
						onloadmore={async () => await loadMore()}
						renderDistance={100}
						getId={(commit) => commit.id}
					>
						{#snippet template(commit, index)}
							<CommitListItem
								disableCommitActions
								type="LocalAndRemote"
								diverged={false}
								commitId={commit.id}
								branchName={branch.branchName}
								commitMessage={commit.message}
								committedAt={commitCommittedAt(commit)}
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
											<ChangedFilesPanel
												title="Changed files"
												{projectId}
												draggableFiles
												selectionId={createCommitSelection({ commitId: commit.id })}
												changes={changesResult.changes.filter(
													(change) =>
														!(change.path in (changesResult.conflictEntries?.entries ?? {})),
												)}
												stats={changesResult.stats ?? undefined}
												conflictEntries={changesResult.conflictEntries}
												autoselect
												allowUnselect={false}
												{onFileClick}
											/>
										{/snippet}
									</ReduxResult>
								{/snippet}
							</CommitListItem>
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
		border: 1px solid var(--border-2);
		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		background-color: var(--bg-1);
	}
</style>
