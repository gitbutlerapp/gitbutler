<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import StackView from '$components/v3/StackView.svelte';
	import { onReorderDragOver } from '$lib/dragging/reordering';
	import { branchesPath } from '$lib/routes/routes.svelte';
	import { type SelectionId } from '$lib/selection/key';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { throttle } from '$lib/utils/misc';
	import { inject } from '@gitbutler/shared/context';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
		selectionId: SelectionId;
		focusedStackId?: string;
	};

	let { projectId, selectedId, stacks, focusedStackId }: Props = $props();

	const [stackService] = inject(StackService);

	let lanesScrollableEl = $state<HTMLDivElement>();
	let lanesScrollableWidth = $state<number>(0);
	let lanesScrollableHeight = $state<number>(0);

	// Pan-to-scroll state
	let isPanning = $state<boolean>(false);
	let panStartX = $state<number>(0);
	let panStartScrollLeft = $state<number>(0);

	let laneWidths = $state<number[]>([]);
	let lineHights = $state<number[]>([]);
	let isNotEnoughHorzSpace = $derived(
		(lanesScrollableWidth ?? 0) < laneWidths.length * (laneWidths[0] ?? 0)
	);
	let visibleIndexes = $state<number[]>([0]);
	let isCreateNewVisible = $state<boolean>(false);

	const [uiState] = inject(UiState);
	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');
	const isDraftStackVisible = $derived(
		isCommitting && exclusiveAction?.type === 'commit' && exclusiveAction?.stackId === undefined
	);

	const SHOW_PAGINATION_THRESHOLD = 1;

	// Pan-to-scroll functions
	function handleMouseDown(e: MouseEvent) {
		if (!lanesScrollableEl) return;

		// Only start panning on left mouse button
		if (e.button !== 0) return;

		const target = e.target as HTMLElement;

		// Exclude clicks on interactive elements
		if (target.closest('button, a, input, select, textarea')) return;
		if (target.closest('[data-remove-from-panning]')) return;

		isPanning = true;
		panStartX = e.clientX;
		panStartScrollLeft = lanesScrollableEl.scrollLeft;

		// Prevent text selection during pan
		e.preventDefault();

		// Add global event listeners
		document.addEventListener('mousemove', handleMouseMove);
		document.addEventListener('mouseup', handleMouseUp);

		// Change cursor
		lanesScrollableEl.style.cursor = 'grabbing';
	}

	function handleMouseMove(e: MouseEvent) {
		if (!isPanning || !lanesScrollableEl) return;

		e.preventDefault();

		const deltaX = e.clientX - panStartX;
		lanesScrollableEl.scrollLeft = panStartScrollLeft - deltaX;
	}

	function handleMouseUp() {
		if (!isPanning || !lanesScrollableEl) return;

		isPanning = false;

		// Remove global event listeners
		document.removeEventListener('mousemove', handleMouseMove);
		document.removeEventListener('mouseup', handleMouseUp);

		// Reset cursor
		lanesScrollableEl.style.cursor = '';
	}

	// Clean up event listeners on component destruction
	$effect(() => {
		return () => {
			document.removeEventListener('mousemove', handleMouseMove);
			document.removeEventListener('mouseup', handleMouseUp);
		};
	});

	// Throttle calls to the reordering code in order to save some cpu cycles.
	const throttledDragOver = throttle(onReorderDragOver, 250);

	// To support visual reordering of stacks we need a copy of the array
	// that can be mutated as the stack is being dragged around.
	let mutableStacks = $state<Stack[]>([]);

	// This is a bit of anti-pattern, and reordering should be better
	// encapsulated such that we don't need this somewhat messy code.
	$effect(() => {
		if (stacks) {
			mutableStacks = stacks;
		}
	});
</script>

{#if isNotEnoughHorzSpace}
	<div class="pagination-container">
		<MultiStackPagination
			length={stacks.length}
			{visibleIndexes}
			{isCreateNewVisible}
			selectedBranchIndex={stacks.findIndex((s) => {
				return s.id === selectedId;
			})}
			onPageClick={(index) => scrollToLane(lanesScrollableEl, index)}
			onCreateNewClick={() => {
				scrollToLane(lanesScrollableEl, stacks.length + 1);
			}}
		/>
	</div>
{/if}

<div
	role="presentation"
	class="lanes-scrollable hide-native-scrollbar"
	class:panning={isPanning}
	bind:this={lanesScrollableEl}
	bind:clientWidth={lanesScrollableWidth}
	bind:clientHeight={lanesScrollableHeight}
	class:multi={stacks.length < SHOW_PAGINATION_THRESHOLD}
	onmousedown={handleMouseDown}
	ondragover={(e) => {
		// This call will actually mutate the array of stacks, showing
		// where the lane will be placed when dropped.
		throttledDragOver(e, mutableStacks);
	}}
	ondrop={() => {
		stackService.updateStackOrder({
			projectId,
			stacks: mutableStacks.map((b, i) => ({ id: b.id, order: i }))
		});
	}}
>
	<StackDraft {projectId} visible={isDraftStackVisible} />

	<!--
	Ideally we wouldn't key on stack id, but the opacity change is done on the
	element being dragged, and therefore stays in place without the key. We
	should find a way of encapsulating the reordering logic better, perhaps
	with some feedback that enables the opacity to become a prop for
	`StackView` instead of being set imperatively in the dragstart handler.
	 -->
	{#each mutableStacks as stack, i (stack.id)}
		<StackView
			{projectId}
			{stack}
			{focusedStackId}
			bind:clientWidth={laneWidths[i]}
			bind:clientHeight={lineHights[i]}
			onVisible={(visible) => {
				if (visible) {
					visibleIndexes = [...visibleIndexes, i];
				} else {
					visibleIndexes = visibleIndexes.filter((index) => index !== i);
				}
			}}
		/>
	{/each}

	<MultiStackOfflaneDropzone
		{projectId}
		viewport={lanesScrollableEl}
		onVisible={(visible) => {
			isCreateNewVisible = visible;
		}}
	>
		{#snippet title()}
			{#if stacks.length === 0}
				No branches in Workspace
			{/if}
		{/snippet}
		{#snippet description()}
			{#if stacks.length === 0}
				Drop files to start a branch,
				<br />
				or apply from the
				<a
					class="pointer-events underline"
					aria-label="Branches view"
					href={branchesPath(projectId)}>Branches view</a
				>
			{:else}
				Drag changes here to
				<br />
				branch off your changes
			{/if}
		{/snippet}
	</MultiStackOfflaneDropzone>
</div>
{#if lanesScrollableEl}
	<Scrollbar viewport={lanesScrollableEl} horz />
{/if}

<style lang="postcss">
	.lanes-scrollable {
		display: flex;
		position: relative;
		flex: 1;
		height: 100%;
		margin: 0 -1px;
		overflow-x: auto;
		overflow-y: hidden;
		cursor: default;
		user-select: none; /* Prevent text selection during pan */
	}

	.lanes-scrollable.panning {
		cursor: grabbing;
	}

	.pagination-container {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		right: 6px;
		bottom: 8px;
	}
</style>
