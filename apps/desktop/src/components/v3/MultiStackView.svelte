<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import BranchLayoutMode from '$components/v3/BranchLayoutMode.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import StackView from '$components/v3/StackView.svelte';
	import { stackLayoutMode, threePointFive } from '$lib/config/uiFeatureFlags';
	import { type SelectionId } from '$lib/selection/key';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
		selectionId: SelectionId;
		focusedStackId?: string;
	};

	const { projectId, selectedId, stacks, selectionId, focusedStackId }: Props = $props();

	let lanesSrollableEl = $state<HTMLDivElement>();
	let lanesScrollableWidth = $state<number>(0);
	let lanesScrollableHeight = $state<number>(0);
	let scrollbar = $state<Scrollbar>();

	let laneWidths = $state<number[]>([]);
	let lineHights = $state<number[]>([]);
	let isNotEnoughHorzSpace = $derived(
		(lanesScrollableWidth ?? 0) < laneWidths.length * (laneWidths[0] ?? 0)
	);
	let isNotEnoughVertSpace = $derived.by(() => {
		const shortenArray = lineHights.slice(0, lineHights.length);
		return lanesScrollableHeight < shortenArray.reduce((acc, height) => acc + height, 0);
	});
	let visibleIndexes = $state<number[]>([0]);
	let isCreateNewVisible = $state<boolean>(false);

	$effect(() => {
		// Explicit scrollbar track size update since changing scroll width
		// does not trigger the resize observer, and changing css does not
		// trigger the mutation observer
		if ($stackLayoutMode) scrollbar?.updateTrack();
	});

	const [uiState] = inject(UiState);
	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const SHOW_PAGINATION_THRESHOLD = 1;
</script>

{#if !$threePointFive}
	<div class="lanes-header" class:no-stacks={stacks.length === 0}>
		{#if stacks.length > 0}
			<div class="title">
				<h3 class="text-14 text-semibold truncate">Applied branches</h3>
				<Badge>{stacks.length}</Badge>
			</div>
			<div class="actions">
				<BranchLayoutMode bind:mode={$stackLayoutMode} />
			</div>
		{:else}
			<div class="title">
				<h3 class="text-14 text-semibold truncate">No branches</h3>
			</div>
		{/if}
		<MultiStackCreateNew {projectId} stackId={selectedId} noStacks={stacks.length === 0} />
	</div>
{/if}

{#if isNotEnoughHorzSpace && isNotEnoughVertSpace}
	<div
		class="pagination-container"
		class:horz={$stackLayoutMode !== 'vertical'}
		class:vert={$stackLayoutMode === 'vertical'}
	>
		<MultiStackPagination
			length={stacks.length}
			{visibleIndexes}
			{isCreateNewVisible}
			selectedBranchIndex={stacks.findIndex((s) => {
				return s.id === selectedId;
			})}
			onPageClick={(index) =>
				scrollToLane(lanesSrollableEl, index, $stackLayoutMode === 'vertical' ? 'vert' : 'horz')}
			onCreateNewClick={() => {
				scrollToLane(
					lanesSrollableEl,
					stacks.length + 1,
					$stackLayoutMode === 'vertical' ? 'vert' : 'horz'
				);
			}}
		/>
	</div>
{/if}

<div class="lanes-viewport">
	<div
		class="lanes-scrollable hide-native-scrollbar"
		bind:this={lanesSrollableEl}
		bind:clientWidth={lanesScrollableWidth}
		bind:clientHeight={lanesScrollableHeight}
		class:multi={$stackLayoutMode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
		class:single={$stackLayoutMode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
		class:vertical={$stackLayoutMode === 'vertical'}
	>
		{#if stacks.length > 0}
			{#each stacks as stack, i}
				<StackView
					{projectId}
					{stack}
					{selectionId}
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
					siblingCount={stacks.length}
				/>
			{/each}

			{#if lanesSrollableEl && $stackLayoutMode !== 'single'}
				<Scrollbar viewport={lanesSrollableEl} horz={$stackLayoutMode !== 'vertical'} />
			{/if}
		{:else if isCommitting}
			<StackDraft {projectId} />
		{:else}
			<div class="no-stacks-placeholder">
				<MultiStackOfflaneDropzone
					viewport={lanesSrollableEl}
					{projectId}
					standalone
					onVisible={(visible) => {
						isCreateNewVisible = visible;
					}}
					isSingleMode={$stackLayoutMode === 'single'}
				/>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.lanes-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 44px;
		padding-left: 12px;
		gap: 10px;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);

		& .title {
			display: flex;
			flex: 1;
			align-items: center;
			overflow: hidden;
			gap: 6px;
		}

		& .actions {
			display: flex;
		}

		&.no-stacks {
			background: transparent;

			& .title {
				color: var(--clr-text-3);
			}
		}
	}

	.lanes-scrollable {
		display: flex;
		height: 100%;
		margin: 0 -1px;

		&.single {
			scroll-snap-type: x mandatory;
		}
		&.single,
		&.multi {
			overflow-x: auto;
		}
		&.vertical {
			flex-direction: column;
			overflow-y: auto;
		}
	}

	.lanes-viewport {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	.pagination-container {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;

		&.horz {
			right: 6px;
			bottom: 8px;
		}

		&.vert {
			right: 8px;
			bottom: 8px;
			transform: rotate(90deg) translateY(100%);
			transform-origin: right bottom;
		}
	}

	.no-stacks-placeholder {
		display: flex;
		position: absolute;
		top: 50%;
		left: 50%;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
		transform: translate(-50%, -50%);

		&:after {
			z-index: -1;
			position: absolute;
			top: calc(50% - 50px);
			left: 50%;
			width: 600px;
			height: 600px;
			transform: translate(-50%, -50%);
			border-radius: 100%;
			background: radial-gradient(var(--clr-bg-2) 0%, oklch(from var(--clr-bg-2) l c h / 0) 70%);
			content: '';
		}
	}
</style>
