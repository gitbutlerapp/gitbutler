<script lang="ts">
	import ChangedFiles from './ChangedFiles.svelte';
	import CommitHeader from './CommitHeader.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		commitKey: CommitKey;
		onclick?: () => void;
	};

	const { projectId, commitKey, onclick }: Props = $props();

	const [stackService] = inject(StackService);
	const commitResult = $derived(
		commitKey.upstream
			? stackService.upstreamCommitById(projectId, commitKey)
			: stackService.commitById(projectId, commitKey)
	);
</script>

<ReduxResult result={commitResult.current}>
	{#snippet children(commit)}
		<ConfigurableScrollableContainer>
			<div class="commit-view">
				<CommitHeader {projectId} {commitKey} {commit} {onclick} />
				<ChangedFiles {projectId} commitId={commitKey.commitId} />
			</div>
		</ConfigurableScrollableContainer>
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		position: relative;
		padding: 14px 16px;
		min-height: 100%;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
		background-color: var(--clr-bg-1);
	}
</style>
