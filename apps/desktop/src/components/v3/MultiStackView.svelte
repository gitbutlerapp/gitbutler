<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import StackView from '$components/v3/StackView.svelte';
	import { type SelectionId } from '$lib/selection/key';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
		selectionId: SelectionId;
		focusedStackId?: string;
	};

	const { projectId, selectedId, stacks, focusedStackId }: Props = $props();

	let lanesScrollableEl = $state<HTMLDivElement>();
	let lanesScrollableWidth = $state<number>(0);
	let lanesScrollableHeight = $state<number>(0);

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

	const [uiState] = inject(UiState);
	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');

	const SHOW_PAGINATION_THRESHOLD = 1;
</script>

{#if isNotEnoughHorzSpace && isNotEnoughVertSpace}
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

<div class="lanes-viewport">
	<div
		class="lanes-scrollable hide-native-scrollbar"
		bind:this={lanesScrollableEl}
		bind:clientWidth={lanesScrollableWidth}
		bind:clientHeight={lanesScrollableHeight}
		class:multi={stacks.length < SHOW_PAGINATION_THRESHOLD}
	>
		{#if isCommitting && stacks.length === 0}
			<StackDraft {projectId} />
		{:else if stacks.length === 0}
			<div class="no-stacks-placeholder">
				<MultiStackOfflaneDropzone
					viewport={lanesScrollableEl}
					{projectId}
					standalone
					onVisible={(visible) => {
						isCreateNewVisible = visible;
					}}
				/>
			</div>
		{:else if stacks.length > 0}
			{#each stacks as stack, i}
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
				viewport={lanesScrollableEl}
				{projectId}
				standalone
				onVisible={(visible) => {
					isCreateNewVisible = visible;
				}}
			/>
			{#if lanesScrollableEl}
				<Scrollbar viewport={lanesScrollableEl} horz />
			{/if}
		{/if}
	</div>
</div>

<style lang="postcss">
	.lanes-scrollable {
		display: flex;
		height: 100%;
		margin: 0 -1px;
		overflow-x: auto;
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
		right: 6px;
		bottom: 8px;
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
