<script lang="ts">
	import BranchCard from '$components/BranchCard.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import NewCommitView from '$components/NewCommitView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import CodegenMessages from '$components/codegen/CodegenMessages.svelte';
	import { messageQueueSlice } from '$lib/codegen/messageQueueSlice';
	import { showError } from '$lib/notifications/toasts';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { CLIENT_STATE } from '$lib/state/clientState.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';
	import { onMount } from 'svelte';
	import { fly } from 'svelte/transition';

	type DraftMode = 'commit' | 'codegen';

	type Props = {
		projectId: string;
		visible?: boolean;
		mode?: DraftMode;
	};

	let { projectId, visible, mode = $bindable('commit') }: Props = $props();

	const DETAILS_RIGHT_PADDING_REM = 1.125;

	const uiState = inject(UI_STATE);
	const projectState = $derived(uiState.project(projectId));
	const stackService = inject(STACK_SERVICE);
	const clientState = inject(CLIENT_STATE);
	const draftBranchName = $derived(uiState.global.draftBranchName);

	let draftPanelEl: HTMLDivElement | undefined = $state();
	let draftCodegenEl: HTMLDivElement | undefined = $state();
	let isCreatingStack = $state(false);

	const newNameQuery = $derived(stackService.newBranchName(projectId));
	const [createNewStack] = stackService.newStack;

	async function handleCodegenSubmit(prompt: string, branchName: string) {
		if (isCreatingStack || !prompt) return;

		isCreatingStack = true;
		try {
			// Create the new stack
			const stack = await createNewStack({
				projectId,
				branch: { name: branchName, order: 0 }
			});

			const stackId = stack.id;
			const finalBranchName = stack.heads[0]?.name;

			if (!stackId || !finalBranchName) {
				throw new Error('Failed to create stack');
			}

			// Get current settings from project state
			const thinkingLevel = projectState.thinkingLevel.current;
			const model = projectState.selectedModel.current;
			const permissionMode = uiState.lane('draft-codegen').permissionMode.current;

			// Add message to the queue for the new stack

			clientState.dispatch(
				messageQueueSlice.actions.upsert({
					projectId,
					stackId,
					head: finalBranchName,
					isProcessing: false,
					messages: [
						{
							prompt,
							thinkingLevel,
							model,
							permissionMode
						}
					]
				})
			);

			uiState.global.draftBranchName.set(undefined);
			projectState.exclusiveAction.set(undefined);

			// Clear the draft prompt
			uiState
				.lane(stackId)
				.selection.set({ branchName: finalBranchName, codegen: true, previewOpen: true });
			setTimeout(() => {
				const element = document.querySelector(`[data-id="${stackId}"]`);
				if (element instanceof HTMLElement) element?.focus();
			}, 100);
		} catch (err: unknown) {
			showError('Failed to create codegen session', err);
		} finally {
			isCreatingStack = false;
		}
	}

	onMount(() => {
		if (draftPanelEl) {
			draftPanelEl.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
	});
</script>

{#if visible}
	<div data-testid={TestId.StackDraft} class="draft-stack">
		<ConfigurableScrollableContainer childrenWrapHeight="100%">
			<div
				class="draft-stack__scroll-wrap"
				bind:this={draftPanelEl}
				style:width={uiState.global.stackWidth.current + 'rem'}
			>
				{#if mode === 'commit'}
					<div class="new-commit-view">
						<NewCommitView {projectId} />
					</div>
				{/if}
				<ReduxResult {projectId} result={newNameQuery.result}>
					{#snippet children(newName)}
						{@const branchName = draftBranchName.current || newName}
						<BranchCard
							type="draft-branch"
							{projectId}
							{branchName}
							isCommitting
							readonly={false}
							lineColor="var(--clr-commit-local)"
						/>
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
		{#if mode === 'codegen'}
			<div
				bind:this={draftCodegenEl}
				in:fly={{ y: 20, duration: 200 }}
				class="codegen-draft"
				data-details="default"
				style:right="{DETAILS_RIGHT_PADDING_REM}rem"
			>
				<ReduxResult {projectId} result={newNameQuery.result}>
					{#snippet children(newName)}
						{@const branchName = draftBranchName.current || newName}
						{@const laneState = uiState.lane('draft-codegen')}
						<CodegenMessages
							{projectId}
							{branchName}
							events={[]}
							permissionRequests={[]}
							laneId="draft-codegen"
							initialPrompt={laneState.prompt.current}
							isStackActive={false}
							onChange={(prompt) => laneState.prompt.set(prompt)}
							onSubmit={async (prompt) => {
								await handleCodegenSubmit(prompt, branchName);
							}}
							onclose={() => {
								projectState.exclusiveAction.set(undefined);
								laneState.prompt.set('');
							}}
						/>
					{/snippet}
				</ReduxResult>
			</div>
			<Resizer
				persistId="resizer-draft-codegen"
				viewport={draftCodegenEl}
				direction="right"
				defaultValue={23}
				minWidth={16}
				maxWidth={64}
			/>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.draft-stack {
		display: flex;
		position: relative;
		flex-shrink: 0;
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

	.codegen-draft {
		display: flex;
		z-index: var(--z-ground);
		flex-shrink: 0;
		flex-direction: column;
		width: 400px;
		height: 100%;
		max-height: calc(100% - 24px);
		margin-top: 12px;
		margin-right: 18px;
		overflow: hidden;
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: 0 10px 30px 0 color(srgb 0 0 0 / 0.16);
	}
</style>
