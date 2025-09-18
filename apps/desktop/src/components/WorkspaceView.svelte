<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Feed from '$components/Feed.svelte';
	import MainViewport from '$components/MainViewport.svelte';
	import MultiStackView from '$components/MultiStackView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import UnassignedView from '$components/UnassignedView.svelte';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { SETTINGS_SERVICE } from '$lib/config/appSettingsV2';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE, type ExclusiveAction } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		scrollToStackId?: string;
		onScrollComplete?: () => void;
	}

	const { projectId, scrollToStackId, onScrollComplete }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const settingsService = inject(SETTINGS_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);

	const selectionId = createWorktreeSelection({ stackId: undefined });
	const worktreeSelection = $derived(idSelection.getById(selectionId));
	const stacksQuery = $derived(stackService.stacks(projectId));
	const projectState = $derived(uiState.project(projectId));
	const settingsStore = $derived(settingsService.appSettings);
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);
	const showingActions = $derived(projectState.showActions.current && canUseActions);
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const baseBranchQuery = $derived(baseBranchService.baseBranch(projectId));
	const baseSha = $derived(baseBranchQuery.response?.baseSha);

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);

	// Ensures that the exclusive action is still valid.
	$effect(() => {
		if (exclusiveAction?.type === 'commit') {
			ensureExclusiveCommitValid(exclusiveAction);
		}
	});

	function ensureExclusiveCommitValid(action: ExclusiveAction & { type: 'commit' }) {
		// We are committing to a stack that has not been created yet
		if (!action.stackId) {
			return;
		}

		const stacks = stackService.stacks(projectId);
		const branch = stackService.branchDetails(projectId, action.stackId, action.branchName);

		$effect(() => {
			const stackFound = stacks.response?.find((s) => s.id === action.stackId);
			if (!stackFound) {
				uiState.project(projectId).exclusiveAction.set(undefined);
			}

			if (!action.branchName) {
				return;
			}

			if (!branch?.response) {
				uiState.project(projectId).exclusiveAction.set(undefined);
				return;
			}

			// If the parentCommitId is not set, we are committing to the top of the stack.
			if (!action.parentCommitId) {
				return;
			}

			// When we're committing to the bottom of the stack we set the
			// commit id to equal the workspace base.
			const hasCommit = branch.response.commits.some((c) => c.id === action.parentCommitId);
			if (!hasCommit && action.parentCommitId !== baseSha) {
				uiState.project(projectId).exclusiveAction.set(undefined);
			}
		});
	}

	let selectionPreviewScrollContainer: HTMLDivElement | undefined = $state();
</script>

{#snippet right()}
	<Feed {projectId} onCloseClick={() => uiState.project(projectId).showActions.set(false)} />
{/snippet}

{#snippet leftPreview()}
	<ConfigurableScrollableContainer
		bind:viewport={selectionPreviewScrollContainer}
		zIndex="var(--z-lifted)"
	>
		<SelectionView
			bottomBorder
			{projectId}
			{selectionId}
			draggableFiles
			scrollContainer={selectionPreviewScrollContainer}
			onclose={() => {
				idSelection.clear(selectionId);
			}}
		/>
	</ConfigurableScrollableContainer>
{/snippet}

<MainViewport
	testId={TestId.WorkspaceView}
	name="workspace"
	leftWidth={{ default: 280, min: 260 }}
	preview={previewOpen ? leftPreview : undefined}
	previewWidth={{ default: 480, min: 220 }}
	right={showingActions ? right : undefined}
	rightWidth={{ default: 320, min: 220 }}
>
	{#snippet left()}
		<UnassignedView {projectId} />
	{/snippet}
	{#snippet middle()}
		<ReduxResult {projectId} result={stacksQuery?.result}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}
			{#snippet children(stacks, { projectId })}
				<MultiStackView {projectId} {stacks} {selectionId} {scrollToStackId} {onScrollComplete} />
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>

<style>
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
