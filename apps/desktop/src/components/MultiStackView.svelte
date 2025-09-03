<script lang="ts">
	import MultiStackOfflaneDropzone from '$components/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/MultiStackPagination.svelte';
	import Scrollbar from '$components/Scrollbar.svelte';
	import StackDraft from '$components/StackDraft.svelte';
	import StackView from '$components/StackView.svelte';
	import { DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
	import { HorizontalPanner } from '$lib/dragging/horizontalPanner';
	import {
		onReorderEnd,
		onReorderMouseDown,
		onReorderStart,
		onDragOver
	} from '$lib/dragging/reordering';
	import { WorkspaceAutoPanner } from '$lib/dragging/workspaceAutoPanner';
	import { branchesPath } from '$lib/routes/routes.svelte';
	import { type SelectionId } from '$lib/selection/key';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { throttle } from '$lib/utils/misc';
	import { inject } from '@gitbutler/core/context';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { flip } from 'svelte/animate';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stacks: Stack[];
		selectionId: SelectionId;
		focusedStackId?: string;
		scrollToStackId?: string;
		onScrollComplete?: () => void;
	};

	let { projectId, stacks, focusedStackId, scrollToStackId, onScrollComplete }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	let lanesScrollableEl = $state<HTMLDivElement>();
	let lanesScrollableWidth = $state<number>(0);
	let lanesScrollableHeight = $state<number>(0);
	let stackElements = $state<Record<string, HTMLElement>>({});

	let laneWidths = $state<number[]>([]);
	let lineHights = $state<number[]>([]);
	let isNotEnoughHorzSpace = $derived(
		(lanesScrollableWidth ?? 0) < laneWidths.length * (laneWidths[0] ?? 0)
	);
	let visibleIndexes = $state<number[]>([0]);
	let isCreateNewVisible = $state<boolean>(false);

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');
	const isDraftStackVisible = $derived(
		isCommitting && exclusiveAction?.type === 'commit' && exclusiveAction?.stackId === undefined
	);

	const SHOW_PAGINATION_THRESHOLD = 1;

	// Throttle calls to the reordering code in order to save some cpu cycles.
	const throttledDragOver = throttle(onDragOver, 25);

	// To support visual reordering of stacks we need a copy of the array
	// that can be mutated as the stack is being dragged around.
	let mutableStacks = $state<Stack[]>([]);

	// Enable panning when a stack is being dragged.
	let draggingStack = $state(false);

	// This is a bit of anti-pattern, and reordering should be better
	// encapsulated such that we don't need this somewhat messy code.
	$effect(() => {
		if (stacks) {
			mutableStacks = stacks;
		}
	});

	const workspaceAutoPanner = $derived(
		lanesScrollableEl ? new WorkspaceAutoPanner(lanesScrollableEl) : undefined
	);

	// Enable panning when anything is being dragged.
	const isDragging = dragStateService.isDragging;
	$effect(() => {
		if ($isDragging || draggingStack) {
			const unsub = workspaceAutoPanner?.enablePanning();
			return () => unsub?.();
		}
	});

	const horizontalPanner = $derived(
		lanesScrollableEl ? new HorizontalPanner(lanesScrollableEl) : undefined
	);

	$effect(() => {
		if (horizontalPanner) {
			const unsub = horizontalPanner.registerListeners();
			return () => unsub?.();
		}
	});

	// Scroll to stack when scrollToStackId is set
	$effect(() => {
		if (scrollToStackId && stacks.length > 0 && lanesScrollableEl) {
			setTimeout(() => {
				const stackEl = stackElements[scrollToStackId];
				if (stackEl) {
					stackEl.scrollIntoView({ behavior: 'smooth' });
				}
				onScrollComplete?.();
			}, 50);
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
				return s.id === focusedStackId;
			})}
			onPageClick={(index) => scrollToLane(lanesScrollableEl, index)}
			onCreateNewClick={() => {
				scrollToLane(lanesScrollableEl, stacks.length + 1);
			}}
		/>
	</div>
{/if}

<div
	class="scrollbar-container hide-native-scrollbar"
	role="presentation"
	bind:this={lanesScrollableEl}
	bind:clientWidth={lanesScrollableWidth}
	bind:clientHeight={lanesScrollableHeight}
	class:multi={stacks.length < SHOW_PAGINATION_THRESHOLD}
	ondrop={() => {
		stackService.updateStackOrder({
			projectId,
			stacks: mutableStacks
				.map((b, i) => (b.id ? { id: b.id, order: i } : undefined))
				.filter(isDefined)
		});
	}}
>
	<div class="lanes-scrollable">
		<StackDraft {projectId} visible={isDraftStackVisible} />

		<!--
	Ideally we wouldn't key on stack id, but the opacity change is done on the
	element being dragged, and therefore stays in place without the key. We
	should find a way of encapsulating the reordering logic better, perhaps
	with some feedback that enables the opacity to become a prop for
	`StackView` instead of being set imperatively in the dragstart handler.
	 -->
		{#each mutableStacks as stack, i (stack.id)}
			<div
				bind:this={stackElements[stack.id || 'branchless']}
				class="reorderable-stack"
				role="presentation"
				animate:flip={{ duration: 150 }}
				onmousedown={onReorderMouseDown}
				ondragstart={(e) => {
					if (!stack.id) return;
					onReorderStart(e, stack.id, () => {
						draggingStack = true;
					});
				}}
				ondragover={(e) => {
					if (!stack.id) return;
					throttledDragOver(e, mutableStacks, stack.id);
				}}
				ondragend={() => {
					draggingStack = false;
					onReorderEnd();
				}}
			>
				<StackView
					{projectId}
					laneId={stack.id || 'banana'}
					stackId={stack.id}
					topBranch={stack.heads.at(0)?.name}
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
			</div>
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
</div>

<style lang="postcss">
	.scrollbar-container {
		display: flex;
		flex: 1;
		height: 100%;
		margin: 0 -1px;
		overflow-x: auto;
		overflow-y: hidden;
	}

	.lanes-scrollable {
		display: flex;
		position: relative;
		height: 100%;
	}

	.pagination-container {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		right: 6px;
		bottom: 8px;
	}

	.reorderable-stack {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: fit-content;
		height: 100%;

		&:first-child {
			border-left: 1px solid var(--clr-border-2);
		}
	}
</style>
