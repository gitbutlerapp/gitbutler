<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import CommitDetails from '$components/v3/CommitDetails.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitLine from '$components/v3/CommitLine.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { CommitStatus } from '$lib/commits/commit';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	type Props = {
		projectId: string;
		commitId: string;
		branchName: string;
		remote?: string;
	};

	const { projectId, commitId, branchName, remote }: Props = $props();

	const [stackService] = inject(StackService);
	const changesResult = $derived(stackService.commitChanges(projectId, commitId));
	const commitResult = $derived(
		stackService.unstackedCommitById(projectId, branchName, commitId, remote)
	);
</script>

<Drawer {projectId}>
	{#snippet header()}
		<div class="commit-view__header text-13">
			<CommitLine commitStatus="Remote" diverged={false} tooltip={CommitStatus.Base} width={24} />
			<div class="commit-view__header-title text-13">
				<span class="text-semibold">Base commit:</span>

				<Tooltip text="Copy commit SHA">
					<button
						type="button"
						class="commit-view__header-sha"
						onclick={() => {
							writeClipboard(commitId, {
								message: 'Commit SHA copied'
							});
						}}
					>
						<span>
							{commitId.substring(0, 7)}
						</span>
						<Icon name="copy-small" /></button
					>
				</Tooltip>
			</div>
		</div>
	{/snippet}

	<div class="commit-view">
		<ReduxResult {projectId} result={commitResult.current}>
			{#snippet children(commit)}
				<CommitHeader commitMessage={commit.message} className="text-14 text-semibold text-body" />
				<CommitDetails {commit} />
			{/snippet}
		</ReduxResult>
	</div>

	{#snippet filesSplitView()}
		<ReduxResult {projectId} result={changesResult.current}>
			{#snippet children(changes)}
				<ChangedFiles
					title="Changed files"
					{projectId}
					selectionId={{ type: 'commit', commitId }}
					{changes}
				/>
			{/snippet}
		</ReduxResult>
	{/snippet}
</Drawer>

<style>
	.commit-view {
		position: relative;
		height: 100%;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.commit-view__header {
		display: flex;
		gap: 8px;
		height: 100%;
		margin-left: -4px;
	}

	.commit-view__header-title {
		align-self: center;
	}

	.commit-view__header-sha {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		text-decoration: dotted underline;
		transition: color var(--transition-fast);
		cursor: pointer;
		color: var(--clr-text-2);

		&:hover {
			color: var(--clr-text-1);
		}
	}
</style>
