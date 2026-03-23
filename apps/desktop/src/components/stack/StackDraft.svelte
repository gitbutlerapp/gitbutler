<script lang="ts">
	import DraftBranchHeader from "$components/branch/DraftBranchHeader.svelte";
	import NewCommitView from "$components/commit/NewCommitView.svelte";
	import AppScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { TestId } from "@gitbutler/ui";
	import { onMount } from "svelte";

	type Props = {
		projectId: string;
		visible?: boolean;
	};

	let { projectId, visible }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const draftBranchName = $derived(uiState.global.draftBranchName);
	const newNameQuery = $derived(stackService.newBranchName(projectId));

	let draftPanelEl: HTMLDivElement | undefined = $state();

	onMount(() => {
		if (draftPanelEl) {
			draftPanelEl.scrollIntoView({ behavior: "smooth", block: "start" });
		}
	});
</script>

{#if visible}
	<div data-testid={TestId.StackDraft} class="draft-stack dotted-pattern">
		<AppScrollableContainer childrenWrapHeight="100%">
			<div
				class="draft-stack__scroll-wrap"
				bind:this={draftPanelEl}
				style:width={uiState.global.stackWidth.current + "rem"}
			>
				<div class="new-commit-view">
					<NewCommitView {projectId} />
				</div>
				<ReduxResult {projectId} result={newNameQuery.result}>
					{#snippet children(newName)}
						{@const branchName = draftBranchName.current || newName}
						<DraftBranchHeader
							{branchName}
							lineColor="var(--clr-commit-local)"
							mode="commit"
							isCommitting
							updateBranchName={(name) => uiState.global.draftBranchName.set(name)}
							isUpdatingName={false}
							failedToUpdateName={false}
						/>
					{/snippet}
				</ReduxResult>
				<Resizer
					persistId="resizer-darft-panel"
					viewport={draftPanelEl}
					direction="right"
					defaultValue={22}
					minWidth={16}
					maxWidth={64}
				/>
			</div>
		</AppScrollableContainer>
	</div>
{/if}

<style lang="postcss">
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
