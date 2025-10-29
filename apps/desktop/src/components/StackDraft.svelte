<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import CommitGoesHere from '$components/CommitGoesHere.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import NewCommitView from '$components/NewCommitView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';
	import { onMount } from 'svelte';

	type Props = {
		projectId: string;
		visible?: boolean;
	};

	const { projectId, visible }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const draftBranchName = $derived(uiState.global.draftBranchName);

	let draftPanelEl: HTMLDivElement | undefined = $state();

	const newNameQuery = $derived(stackService.newBranchName(projectId));

	onMount(() => {
		if (draftPanelEl) {
			draftPanelEl.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
	});
</script>

{#if visible}
	<div
		bind:this={draftPanelEl}
		data-testid={TestId.StackDraft}
		class="draft-stack"
		style:width={uiState.global.stackWidth.current + 'rem'}
	>
		<ConfigurableScrollableContainer childrenWrapHeight="100%">
			<div class="draft-stack__scroll-wrap">
				<div class="new-commit-view">
					<NewCommitView {projectId} />
				</div>
				<ReduxResult {projectId} result={newNameQuery.result}>
					{#snippet children(newName)}
						{@const branchName = draftBranchName.current || newName}
						<BranchCard
							type="draft-branch"
							{projectId}
							{branchName}
							readonly={false}
							lineColor="var(--clr-commit-local)"
						>
							{#snippet branchContent()}
								<CommitGoesHere commitId={undefined} selected draft />
							{/snippet}
						</BranchCard>
					{/snippet}
				</ReduxResult>
				<Resizer
					persistId="resizer-darft-panel"
					viewport={draftPanelEl}
					direction="right"
					defaultValue={23}
					minWidth={16}
					maxWidth={64}
				/>
			</div>
		</ConfigurableScrollableContainer>
	</div>

	<style lang="postcss">
		.draft-stack {
			display: flex;
			position: relative;
			flex-shrink: 0;
			flex-direction: column;
			min-height: 100%;
			border-right: 1px solid var(--clr-border-2);
			animation: appear-in 0.2s ease-in-out forwards;
		}
		.draft-stack__scroll-wrap {
			position: relative;
			min-height: 100%;
			padding: 12px;
		}
		.new-commit-view {
			margin-bottom: 12px;
			overflow: hidden;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-ml);
			background-color: var(--clr-bg-1);
		}

		@keyframes appear-in {
			from {
				transform: translateX(-20px);
				opacity: 0;
			}
			to {
				transform: translateX(0);
				opacity: 1;
			}
		}
	</style>
{/if}
