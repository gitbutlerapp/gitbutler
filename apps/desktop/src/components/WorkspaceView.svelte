<script lang="ts">
	import FullviewLoading from '$components/FullviewLoading.svelte';
	import MainViewport from '$components/MainViewport.svelte';
	import MultiDiffView from '$components/MultiDiffView.svelte';
	import MultiStackView from '$components/MultiStackView.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import UnassignedView from '$components/UnassignedView.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
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
	const uncommittedService = inject(UNCOMMITTED_SERVICE);

	const selectionId = createWorktreeSelection({ stackId: undefined });
	const worktreeSelection = $derived(idSelection.getById(selectionId));
	const stacksQuery = $derived(stackService.stacks(projectId));

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);

	// Transform unassigned changes to SelectedFile[] format
	const unassignedChanges = $derived(uncommittedService.getChangesByStackId(null));

	let multiDiffView = $state<MultiDiffView>();
	let startIndex = $state(0);
</script>

{#snippet leftPreview()}
	<MultiDiffView
		{projectId}
		{startIndex}
		selectionId={{ type: 'worktree' }}
		stackId={undefined}
		changes={unassignedChanges}
		bind:this={multiDiffView}
		draggable={true}
		selectable={false}
		showBorder={false}
		showRoundedEdges={false}
		onclose={() => {
			idSelection.clear(selectionId);
		}}
	/>
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
		<UnassignedView
			{projectId}
			onFileClick={(index) => {
				startIndex = index;
				multiDiffView?.jumpToIndex(index);
			}}
		/>
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
