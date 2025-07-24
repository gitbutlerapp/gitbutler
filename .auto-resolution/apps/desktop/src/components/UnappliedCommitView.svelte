<script lang="ts">
	import ChangedFiles from '$components/ChangedFiles.svelte';
	import CommitDetails from '$components/CommitDetails.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		projectId: string;
		commitId: string;
		onclose?: () => void;
	};

	const { projectId, commitId, onclose }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const changesResult = $derived(stackService.commitChanges(projectId, commitId));
	const commitResult = $derived(stackService.commitDetails(projectId, commitId));
</script>

<ReduxResult {projectId} result={commitResult.current}>
	{#snippet children(commit)}
		<Drawer {onclose} bottomBorder>
			{#snippet header()}
				<CommitTitle
					truncate
					commitMessage={commit.message}
					className="text-14 text-semibold text-body"
				/>
			{/snippet}

			<div class="commit-view">
				<CommitDetails {commit} rewrap={$rewrapCommitMessage} />
			</div>
		</Drawer>
		<ReduxResult {projectId} result={changesResult.current}>
			{#snippet children(changes)}
				<ChangedFiles
					title="Changed files"
					active
					autoselect
					{projectId}
					selectionId={{ type: 'commit', commitId }}
					changes={changes.changes}
				/>
			{/snippet}
		</ReduxResult>
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		height: 100%;
		padding: 14px;
		gap: 14px;
	}

	.commit-view__header {
		display: flex;
		height: 100%;
		margin-left: -4px;
		gap: 8px;
	}

	.commit-view__header-title {
		align-self: center;
	}

	.commit-view__header-sha {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		color: var(--clr-text-2);
		text-decoration: dotted underline;
		cursor: pointer;
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
		}
	}
</style>
