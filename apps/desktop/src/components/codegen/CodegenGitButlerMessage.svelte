<script lang="ts">
	import CommitDetails from '$components/CommitDetails.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import type { GitButlerUpdate } from '$lib/codegen/types';

	interface Props {
		projectId: string;
		message: GitButlerUpdate;
	}

	let { projectId, message }: Props = $props();

	const stackService = inject(STACK_SERVICE);
</script>

{#if message.type === 'commitCreated'}
	<div class="system-message text-13">
		<p>New commit{message.commitIds.length > 1 ? 's' : ''} created!</p>
		{#each message.commitIds as commitId}
			{@const { stackId } = message}
			{@const commit = stackService.commitById(projectId, stackId, commitId)}
			<ReduxResult {projectId} {stackId} result={commit.result}>
				{#snippet children(commit)}
					<div class="commit-bubble">
						<CommitDetails {commit} />
					</div>
				{/snippet}
				{#snippet empty()}
					Commit {commitId.slice(0, 7)} not found
				{/snippet}
			</ReduxResult>
		{/each}
	</div>
{/if}

<style lang="postcss">
	.system-message {
		display: flex;
		flex-direction: column;
		padding: 12px 0;
		gap: 12px;
	}

	.commit-bubble {
		overflow: hidden;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-2);
	}
</style>
