<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import DependencyService from '$lib/dependencies/dependencyService.svelte';
	import { REJECTTION_REASONS, type RejectionReason } from '$lib/stacks/stackService.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import HunkDiffBody from '@gitbutler/ui/hunkDiff/HunkDiffBody.svelte';
	import { parseHunk } from '@gitbutler/ui/utils/diffParsing';
	import type { CommitFailedModalState } from '$lib/state/uiState.svelte';

	type Props = {
		data: CommitFailedModalState;
	};

	const { data }: Props = $props();

	const [worktreeService, dependencyService] = inject(WorktreeService, DependencyService);

	type ReasonGroup = {
		reason: RejectionReason;
		reasonReadable: string;
		paths: string[];
	};

	function getReadableRejectionReason(reason: RejectionReason): string {
		switch (reason) {
			case 'cherryPickMergeConflict':
				return 'Cherry-pick merge conflict';
			case 'noEffectiveChanges':
				return 'No effective changes';
			case 'workspaceMergeConflict':
				return 'Workspace merge conflict';
			case 'worktreeFileMissingForObjectConversion':
				return 'Worktree file missing for object conversion';
			case 'fileToLargeOrBinary':
				return 'File too large or binary';
			case 'pathNotFoundInBaseTree':
				return 'Path not found in base tree';
			case 'unsupportedDirectoryEntry':
				return 'Unsupported directory entry';
			case 'unsupportedTreeEntry':
				return 'Unsupported tree entry';
			case 'missingDiffSpecAssociation':
				return 'Missing diff spec association';
		}
	}

	function groupByReason(data: CommitFailedModalState): ReasonGroup[] {
		const grouped: Partial<Record<RejectionReason, string[]>> = {};

		for (const [path, reason] of Object.entries(data.pathsToRejectedChanges)) {
			if (!grouped[reason]) {
				grouped[reason] = [];
			}
			grouped[reason].push(path);
		}

		const result: ReasonGroup[] = [];
		for (const reason of REJECTTION_REASONS) {
			const paths = grouped[reason];
			if (!paths) continue;
			result.push({ reason, reasonReadable: getReadableRejectionReason(reason), paths });
		}

		return result;
	}

	function reasonRelatedToDependencyInfo(reason: RejectionReason): boolean {
		return reason === 'cherryPickMergeConflict' || reason === 'workspaceMergeConflict';
	}

	const changesTimestamp = $derived(worktreeService.getChangesTimeStamp(data.projectId));

	const groupedData = groupByReason(data);
</script>

{#snippet fileEntry(path: string, reason: RejectionReason)}
	{#if reasonRelatedToDependencyInfo(reason) && changesTimestamp.current !== undefined}
		<!-- In some cases, the dependency information is relevant to the cause of commit rejection.
		 Show the relevant diff locks in that case. -->
		{@const fileDependencies = dependencyService.fileDependencies(
			data.projectId,
			changesTimestamp.current,
			path
		)}
		<div class="commit-failed__file-entry">
			<p class="text-13 text-semibold">{path}</p>
			<div class="commit-failed__file-entry-dependencies">
				<ReduxResult projectId={data.projectId} result={fileDependencies.current}>
					{#snippet children(fileDependencies)}
						{#each fileDependencies.dependencies as dependency}
							{@const hunk = parseHunk(dependency.hunk.diff)}
							<div class="commit-failed__file-entry-dependencies-diff">
								<table class="table__section">
									<HunkDiffBody content={hunk.contentSections} filePath={path} />
								</table>
							</div>
							Depends on:
							<br />
							{#each dependency.locks as lock}
								<div class="commit-failed__file-entry-dependency-lock">
									<p>Stack Id {lock.stackId}</p>
									<p>Commit Id {lock.commitId}</p>
								</div>
							{/each}
						{/each}
					{/snippet}
				</ReduxResult>
			</div>
		</div>
	{:else}
		<!-- If the dependency information is not relevant, just display the path -->
		<p class="text-13 text-semibold">{path}</p>
	{/if}
{/snippet}

<div class="commit-failed__wrapper">
	<div class="commit-failed__description">
		{#if data.newCommitId}
			<p>
				A commit could be created with SHA <b>{data.newCommitId.substring(0, 7)}</b>
				in branch <b>{data.targetBranchName}</b> but some changes could not be fully committed:
			</p>
		{:else}
			<p>Commit could not be created because of the following reasons:</p>
		{/if}
	</div>
	<ConfigurableScrollableContainer>
		<div class="commit-failed">
			<div class="commit-failed__reasons">
				{#each groupedData as { reason, paths, reasonReadable } (reason)}
					<div class="commit-failed__reason">
						<p class="text-bold text-14">
							{reasonReadable}
						</p>

						<div class="commit-failed__reason-file-list">
							{#each paths as path (path)}
								{@render fileEntry(path, reason)}
							{/each}
						</div>
					</div>
				{/each}
			</div>
		</div>
	</ConfigurableScrollableContainer>
</div>

<style lang="postcss">
	.commit-failed__wrapper {
		display: flex;
		flex-direction: column;
		height: 320px;

		gap: 32px;
	}

	.commit-failed {
		display: flex;
		flex-direction: column;

		gap: 32px;
	}

	.commit-failed__reasons {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.commit-failed__reason {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.commit-failed__reason-file-list {
		display: flex;
		flex-direction: column;

		padding-left: 8px;
		gap: 8px;
	}

	.commit-failed__file-entry {
		display: flex;
		flex-direction: column;

		gap: 4px;
	}

	.commit-failed__file-entry-dependencies-diff {
		overflow: hidden;
		border: 1px solid var(--clr-diff-count-border);
		border-radius: var(--radius-m);
	}

	.table__section {
		width: 100%;
		border-collapse: separate;
		border-spacing: 0;
	}

	.commit-failed__file-entry-dependency-lock {
		display: flex;
		gap: 4px;
	}
</style>
