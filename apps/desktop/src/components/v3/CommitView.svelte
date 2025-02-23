<script lang="ts">
	import StackDetailsCommitHeader from './StackDetailsCommitHeader.svelte';
	import StackDetailsFileList from './StackDetailsFileList.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { branchPath } from '$lib/routes/routes.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { CommitKey } from '$lib/commits/commit';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		commitKey: CommitKey;
	};

	const { projectId, stackId, branchName, commitKey }: Props = $props();

	const [stackService] = inject(StackService);
	const commit = $derived(stackService.commitById(projectId, commitKey).current);
</script>

<ReduxResult result={commit}>
	{#snippet children(commit)}
		<div class="commit-view">
			<div>
				<button
					type="button"
					class="exit-btn"
					onclick={() => goto(branchPath(projectId, stackId, branchName))}
				>
					<Icon name="cross" />
				</button>
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

	.exit-btn {
		position: absolute;
		top: 8px;
		right: 8px;
	}

	.body {
		display: flex;
		flex-direction: column;
	}
</style>
