<script lang="ts">
	import CommitDetails from '$components/CommitDetails.svelte';
	import CommitTitle from '$components/CommitTitle.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { rewrapCommitMessage } from '$lib/config/uiFeatureFlags';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	type Props = {
		projectId: string;
		commitId: string;
		onclose?: () => void;
	};

	const { projectId, commitId, onclose }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const commitQuery = $derived(stackService.commitDetails(projectId, commitId));
</script>

<ReduxResult {projectId} result={commitQuery.result}>
	{#snippet children(commit)}
		<Drawer {onclose} noshrink rounded persistId="unapplied-commit-drawer-{projectId}-{commitId}">
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
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		height: 100%;
		gap: 14px;
	}
</style>
