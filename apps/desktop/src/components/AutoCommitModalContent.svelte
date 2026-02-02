<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { ACTION_SERVICE } from '$lib/actions/actionService.svelte';
	import { CLIPBOARD_SERVICE } from '$lib/backend/clipboard';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		ModalFooter,
		ModalHeader,
		ScrollableContainer,
		SimpleCommitRow
	} from '@gitbutler/ui';
	import { untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { AutoCommitModalState } from '$lib/state/uiState.svelte';
	import type { Action } from '@gitbutler/core/api';

	type Props = {
		data: AutoCommitModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const actionService = inject(ACTION_SERVICE);
	const stackService = inject(STACK_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let events = $state<Action.AutoCommitEvent[]>([]);
	const commitMessageMap = new SvelteMap<string, string>();

	// Listen for auto-commit events and update commit messages accordingly
	$effect(() => {
		const unlisten = actionService.listenForAutoCommit(data.projectId, (event) => {
			untrack(() => {
				events = [...events, event];
			});

			if (event.type === 'commitGeneration') {
				// Accumulate the tokens into a commit message for the corresponding parent commit ID
				const untrackedMap = untrack(() => commitMessageMap);
				const existingMessage = untrackedMap.get(event.parent_commit_id) || '';
				untrackedMap.set(event.parent_commit_id, existingMessage + event.token);
			}
		});

		return () => {
			unlisten();
		};
	});

	function findTheLastCommitGeneration(events: Action.AutoCommitEvent[]): string | undefined {
		for (let i = events.length - 1; i >= 0; i--) {
			const event = events[i];
			if (!event) continue;
			if (event.type === 'commitGeneration') {
				return event.parent_commit_id;
			}
		}
		return undefined;
	}

	const totalSteps = $derived(events.find((e) => e.type === 'started')?.steps_length);
	const commitsCreated = $derived(
		events.filter((e) => e.type === 'commitSuccess').map((e) => e.commit_id)
	);
	const parentOfCommitBeingGenerated = $derived(findTheLastCommitGeneration(events));
	const isDone = $derived(events.some((e) => e.type === 'completed'));
</script>

<ModalHeader type="info">Auto Commit Changes</ModalHeader>
<div class="auto-commit-modal__modal-content">
	{#if parentOfCommitBeingGenerated && !isDone}
		{@const currentCommitMessage = commitMessageMap.get(parentOfCommitBeingGenerated)}
		<p class="text-13 text-body auto-commit-modal__message-generation">{currentCommitMessage}</p>
	{/if}
	<!-- List of created commits -->
	{#if commitsCreated.length > 0}
		<div class="auto-commit-modal__scroll-wrap">
			<ScrollableContainer maxHeight="16.5rem">
				{#each commitsCreated as commitId (commitId)}
					{@const commit = stackService.commitDetails(data.projectId, commitId)}
					<ReduxResult projectId={data.projectId} result={commit.result}>
						{#snippet children(commit)}
							{@const commitTitle = commit.message.split('\n')[0] ?? 'No commit message'}
							{@const date = new Date(Number(commit.createdAt))}

							<SimpleCommitRow
								title={commitTitle}
								sha={commit.id}
								{date}
								author={commit.author.name}
								onCopy={() => clipboardService.write(commit.id, { message: 'Commit hash copied' })}
							/>
						{/snippet}
					</ReduxResult>
				{/each}
			</ScrollableContainer>
		</div>
	{/if}
	<!-- Progress bar -->
	{#if totalSteps !== undefined && totalSteps > 1}
		<div class="auto-commit-modal__progress-container">
			<div
				class="auto-commit-modal__progress-bar"
				style:width="{(commitsCreated.length / totalSteps) * 100}%"
			></div>
		</div>
		<p class="auto-commit-modal__progress-text">
			{#if isDone}
				Done!
			{:else}
				Processing {commitsCreated.length} of {totalSteps} steps
			{/if}
		</p>
	{/if}
</div>
<ModalFooter>
	<Button style={isDone ? 'pop' : undefined} kind={isDone ? 'solid' : 'outline'} onclick={close}>
		{isDone ? 'Done' : 'Close'}
	</Button>
</ModalFooter>

<style>
	.auto-commit-modal__message-generation {
		white-space: pre-wrap;
	}

	.auto-commit-modal__modal-content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px 16px;
		gap: 1rem;
	}

	.auto-commit-modal__progress-container {
		width: 100%;
		height: 8px;
		overflow: hidden;
		border-radius: 4px;
		background-color: var(--clr-bg-2);
	}

	.auto-commit-modal__progress-bar {
		height: 100%;
		border-radius: 4px;
		background-color: var(--clr-theme-pop-element);
		transition: width 0.3s ease;
	}

	.auto-commit-modal__progress-text {
		color: var(--clr-text-2);
		font-size: 12px;
		text-align: center;
	}

	.auto-commit-modal__scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
