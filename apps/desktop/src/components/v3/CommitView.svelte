<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		commitKey: CommitKey;
		onClose: () => void;
	};

	const { projectId, commitKey, onClose }: Props = $props();

	const [stackService] = inject(StackService);
	const commit = $derived(stackService.commitById(projectId, commitKey).current);
</script>

<ReduxResult result={commit}>
	{#snippet children(commit)}
		<div class="commit-view">
			<div>
				<Button
					type="button"
					kind="ghost"
					class="exit-btn"
					icon="cross"
					size="tag"
					onclick={onClose}
				/>
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

		background-color: var(--clr-bg-1);
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
