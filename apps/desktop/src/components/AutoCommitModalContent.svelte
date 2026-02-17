<script lang="ts">
	import ConfigurableScrollableContainer from "$components/ConfigurableScrollableContainer.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { ACTION_SERVICE } from "$lib/actions/actionService.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import {
		ModalHeader,
		ScrollableContainer,
		SimpleCommitRow,
		SimpleCommitRowSkeleton,
	} from "@gitbutler/ui";
	import { untrack } from "svelte";
	import { SvelteMap } from "svelte/reactivity";
	import type { AutoCommitModalState } from "$lib/state/uiState.svelte";
	import type { Action } from "@gitbutler/core/api";

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

	// Listen for auto-commit events and update commit messages
	$effect(() => {
		const unlisten = actionService.listenForAutoCommit(data.projectId, (event) => {
			untrack(() => {
				events = [...events, event];
			});

			if (event.type === "commitGeneration") {
				const untrackedMap = untrack(() => commitMessageMap);
				const currentMessage = untrackedMap.get(event.parent_commit_id) || "";
				untrackedMap.set(event.parent_commit_id, currentMessage + event.token);
			}
		});

		return unlisten;
	});

	// Check if generation is complete for a parent commit
	function isGenerationComplete(
		parentCommitId: string | undefined,
		events: Action.AutoCommitEvent[],
	): boolean {
		if (!parentCommitId) return false;

		for (let i = events.length - 1; i >= 0; i--) {
			const event = events[i];
			if (!event) continue;

			if (event.type === "commitGeneration" && event.parent_commit_id === parentCommitId) {
				// Found the generation, now check if there's a success after it
				for (let j = i + 1; j < events.length; j++) {
					const nextEvent = events[j];
					if (nextEvent?.type === "commitSuccess") {
						return true;
					}
				}
				return false;
			}
		}
		return false;
	}

	const totalSteps = $derived(events.find((e) => e.type === "started")?.steps_length);
	const commitsCreated = $derived(
		events.filter((e) => e.type === "commitSuccess").map((e) => e.commit_id),
	);
	const parentOfCommitBeingGenerated = $derived(
		events
			.slice()
			.reverse()
			.find((e) => e.type === "commitGeneration")?.parent_commit_id,
	);
	const isCurrentGenerationComplete = $derived(
		isGenerationComplete(parentOfCommitBeingGenerated, events),
	);
	const isDone = $derived(events.some((e) => e.type === "completed"));

	let isScrollTopVisible = $state(true);
</script>

<div class="auto-commit-modal__wrapper">
	<ModalHeader sticky={!isScrollTopVisible} closeButton={isDone} oncloseclick={close}
		>Auto commit changes</ModalHeader
	>

	<ConfigurableScrollableContainer
		onscrollTop={(visible) => {
			isScrollTopVisible = visible;
		}}
	>
		<div class="auto-commit-modal__modal-content">
			{#if commitsCreated.length === 0 && !parentOfCommitBeingGenerated}
				<div class="auto-commit-modal__scroll-wrap">
					<SimpleCommitRowSkeleton />
				</div>
			{:else}
				<div class="auto-commit-modal__scroll-wrap">
					<ScrollableContainer>
						{#each commitsCreated as commitId (commitId)}
							{@const commit = stackService.commitDetails(data.projectId, commitId)}
							<ReduxResult projectId={data.projectId} result={commit.result}>
								{#snippet children(commit)}
									{@const commitTitle = commit.message.split("\n")[0] ?? "No commit message"}
									{@const date = new Date(Number(commit.createdAt))}

									<SimpleCommitRow
										title={commitTitle}
										sha={commit.id}
										isDone
										{date}
										author={commit.author.name}
										onCopy={() =>
											clipboardService.write(commit.id, { message: "Commit hash copied" })}
									/>
								{/snippet}

								{#snippet loading()}
									<SimpleCommitRowSkeleton />
								{/snippet}
							</ReduxResult>
						{/each}

						{#if parentOfCommitBeingGenerated && !isDone && !isCurrentGenerationComplete}
							{@const currentMessage = commitMessageMap.get(parentOfCommitBeingGenerated)}
							{@const title = currentMessage?.split("\n")[0] ?? "Generating commit message..."}
							<SimpleCommitRow {title} sha="..." date={new Date()} aiMessage={currentMessage} />
						{/if}

						{#if totalSteps && totalSteps > 1}
							{@const showingGenerating =
								parentOfCommitBeingGenerated && !isDone && !isCurrentGenerationComplete}
							{@const skeletonCount =
								totalSteps - commitsCreated.length - (showingGenerating ? 1 : 0)}
							{#each Array(skeletonCount) as _}
								<SimpleCommitRowSkeleton />
							{/each}
						{/if}
					</ScrollableContainer>
				</div>
			{/if}
		</div>
	</ConfigurableScrollableContainer>
</div>

<style>
	.auto-commit-modal__wrapper {
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.auto-commit-modal__modal-content {
		display: flex;
		flex-direction: column;
		padding: 0 16px 16px 16px;
		gap: 1rem;
	}

	.auto-commit-modal__scroll-wrap {
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
