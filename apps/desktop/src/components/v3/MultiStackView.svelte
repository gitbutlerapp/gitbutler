<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import StackView from '$components/v3/StackView.svelte';
	import { branchesPath } from '$lib/routes/routes.svelte';
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
	let visibleIndexes = $state<number[]>([0]);
	let isCreateNewVisible = $state<boolean>(false);

	const [uiState] = inject(UiState);
	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');

	const SHOW_PAGINATION_THRESHOLD = 1;
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
	class="lanes-scrollable hide-native-scrollbar"
	bind:this={lanesScrollableEl}
	bind:clientWidth={lanesScrollableWidth}
	bind:clientHeight={lanesScrollableHeight}
	class:multi={stacks.length < SHOW_PAGINATION_THRESHOLD}
>
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

	{#if isCommitting && exclusiveAction?.stackId === undefined}
		<StackDraft {projectId} />
	{/if}
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
					class="pointer-events underline-dotted"
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
	}

	.pagination-container {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		right: 6px;
		bottom: 8px;
	}
</style>
