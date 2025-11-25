<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import MainViewport from '$components/MainViewport.svelte';
	import MultiStackView from '$components/MultiStackView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SelectionView from '$components/SelectionView.svelte';
	import UnassignedView from '$components/UnassignedView.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { TestId } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		scrollToStackId?: string;
		onScrollComplete?: () => void;
	}

	const { projectId, scrollToStackId, onScrollComplete }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const selectionId = createWorktreeSelection({ stackId: undefined });
	const worktreeSelection = $derived(idSelection.getById(selectionId));
	const stacksQuery = $derived(stackService.stacks(projectId));

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);

	let selectionPreviewScrollContainer: HTMLDivElement | undefined = $state();
</script>

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
				idSelection.clearPreview(selectionId);
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
	rightWidth={{ default: 320, min: 220 }}
>
	{#snippet left()}
		<UnassignedView {projectId} />
	{/snippet}
	{#snippet middle()}
		<ReduxResult {projectId} result={stacksQuery?.result}>
			{#snippet loading()}
				<FullviewLoading />
			{/snippet}
			{#snippet children(stacks, { projectId })}
				<MultiStackView {projectId} {stacks} {selectionId} {scrollToStackId} {onScrollComplete} />
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>
