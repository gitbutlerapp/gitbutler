<script lang="ts">
	import CommitDetails from '$components/CommitDetails.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';
	import type { GitButlerUpdate } from '$lib/codegen/types';

	interface Props {
		projectId: string;
		message: GitButlerUpdate;
	}

	let { projectId, message }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let expanded = $state(false);
</script>

{#if message.type === 'commitCreated'}
	<div class="system-message">
		<button
			type="button"
			class="tool-btn"
			class:expanded
			onclick={() => {
				expanded = !expanded;
			}}
		>
			<div class="tool-btn__arrow" class:expanded>
				<Icon name="chevron-right" />
			</div>

			<span class="tool-btn__label text-13 text-semibold m-r-2">
				New commit{message.commitIds.length > 1 ? 's' : ''} created
			</span>

			{#if !expanded}
				{#each message.commitIds as commitId}
					<div class="commit-hash">
						<Icon name="commit" color="var(--clr-text-3)" />
						<span>{commitId.slice(0, 7)}</span>
					</div>
				{/each}
			{/if}
		</button>

		{#if expanded}
			{#each message.commitIds as commitId}
				{@const commit = stackService.commitDetails(projectId, commitId)}
				<div class="stack-v gap-8">
					<ReduxResult {projectId} result={commit.result}>
						{#snippet children(commit)}
							<div class="commit-bubble">
								<CommitDetails {commit} />
							</div>
						{/snippet}
						{#snippet empty()}
							<div class="commit-not-found text-12">
								<Icon name="error-small" color="var(--clr-text-2)" />
								<span>Commit {commitId.slice(0, 7)} not found</span>
							</div>
						{/snippet}
					</ReduxResult>
				</div>
			{/each}
		{/if}
	</div>
{/if}

<style lang="postcss">
	.system-message {
		display: flex;
		flex-direction: column;
		padding: 12px 0;
		gap: 12px;
		user-select: text;
	}

	.commit-bubble {
		max-width: 520px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.commit-not-found {
		display: flex;
		align-items: center;
		width: fit-content;
		padding: 12px;
		gap: 6px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.tool-btn {
		display: flex;
		align-items: center;
		width: fit-content;
		gap: 6px;
		user-select: none;

		&:hover {
			.tool-btn__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.tool-btn__arrow {
		display: flex;
		margin-left: -2px;
		color: var(--clr-text-3);
		transition: transform var(--transition-medium);

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.tool-btn__label {
		white-space: nowrap;
	}

	.commit-hash {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		font-size: 0.78rem;
		line-height: 1;
		font-family: var(--font-mono);
	}
</style>
