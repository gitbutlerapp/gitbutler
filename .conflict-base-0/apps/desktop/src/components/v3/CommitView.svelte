<script lang="ts">
	import ChangedFiles from './ChangedFiles.svelte';
	import CommitHeader from './CommitHeader.svelte';
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
				<CommitHeader {projectId} {commitKey} {commit} {onclick} />
			</div>
			<div class="body">
				<ChangedFiles {projectId} {commit} />
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
