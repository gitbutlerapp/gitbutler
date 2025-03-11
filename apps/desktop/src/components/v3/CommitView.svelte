<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		commitKey: CommitKey;
	};

	const { projectId, commitKey }: Props = $props();

	const [stackService] = inject(StackService);
	const commitResult = $derived(stackService.commitById(projectId, commitKey));
</script>

<ReduxResult result={commitResult.current}>
	{#snippet children(commit)}
		<div class="commit-view">
			<div>
				<StackDetailsCommitHeader {projectId} {commitKey} {commit} />
			</div>
			<div class="body">
				<StackDetailsFileList {projectId} {commit} />
			</div>
		</div>
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
