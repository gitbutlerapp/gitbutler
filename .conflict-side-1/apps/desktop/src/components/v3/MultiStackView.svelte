<script lang="ts">
	import BranchLayoutMode, { type Layout } from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Scrollbar from '@gitbutler/ui/scroll/Scrollbar.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
	};

	const { projectId, selectedId, stacks }: Props = $props();
	let mode = $derived(persisted<Layout>('multi', 'branch-layout'));

	let lanesContentEl = $state<HTMLElement>();
	let lanesContentWidth = $state<number>(0);
	let lanesContentHeight = $state<number>(0);
	let scrollbar = $state<Scrollbar>();

	let laneWidths = $state<number[]>([]);
	let lineHights = $state<number[]>([]);
	let isNotEnoughHorzSpace = $derived(
		(lanesContentWidth ?? 0) < (laneWidths.length - 1) * (laneWidths[0] ?? 0)
	);
	let isNotEnoughVertSpace = $derived.by(() => {
		const shortenArray = lineHights.slice(0, lineHights.length - 1);
		return lanesContentHeight < shortenArray.reduce((acc, height) => acc + height, 0);
	});
	let visibleIndexes = $state<number[]>([0]);

	$effect(() => {
		// Explicit scrollbar track size update since changing scroll width
		// does not trigger the resize observer, and changing css does not
		// trigger the mutation observer
		if ($mode) scrollbar?.updateTrack();
	});

	const [uiState] = inject(UiState);
	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const SHOW_PAGINATION_THRESHOLD = 1;
</script>

<div class="lanes">
	<div class="lanes-header">
		<div class="title">
			<h3 class="text-14 text-semibold truncate">Applied branches</h3>
			{#if stacks.length > 0}
				<Badge>{stacks.length}</Badge>
			{/if}
		</div>
		<div class="actions">
			<BranchLayoutMode bind:mode={$mode} />
		</div>
		<MultiStackCreateNew {projectId} stackId={selectedId} noStacks={stacks.length === 0} />
	</div>

	<div
		class="lanes-content hide-native-scrollbar dotted-pattern"
		bind:this={lanesContentEl}
		bind:clientWidth={lanesContentWidth}
		bind:clientHeight={lanesContentHeight}
		class:multi={$mode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
		class:single={$mode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
		class:vertical={$mode === 'vertical'}
	>
		{#if isNotEnoughHorzSpace && isNotEnoughVertSpace}
			<div
				class="pagination-container"
				class:horz={$mode !== 'vertical'}
				class:vert={$mode === 'vertical'}
			>
				<MultiStackPagination
					length={stacks.length}
					{visibleIndexes}
					selectedBranchIndex={stacks.findIndex((s) => {
						return s.id === selectedId;
					})}
					onclick={(index) =>
						scrollToLane(lanesContentEl, index, $mode === 'vertical' ? 'vert' : 'horz')}
				/>
			</div>
		{/if}

		{#if stacks.length > 0}
			{#each stacks as stack, i}
				{@const active = stack.id === projectState.stackId.current}
				<div
					class="lane"
					class:multi={$mode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
					class:single={$mode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
					class:vertical={$mode === 'vertical'}
					data-id={stack.id}
					bind:clientWidth={laneWidths[i]}
					bind:clientHeight={lineHights[i]}
					data-testid={TestId.Stack}
					data-testid-stack={stack.heads.at(0)?.name}
					use:intersectionObserver={{
						callback: (entry) => {
							if (entry?.isIntersecting) {
								visibleIndexes = [...visibleIndexes, i];
							} else {
								visibleIndexes = visibleIndexes.filter((index) => index !== i);
							}
						},
						options: {
							threshold: 0.5,
							root: lanesContentEl
						}
					}}
				>
					<BranchList
						isVerticalMode={$mode === 'vertical'}
						{projectId}
						stackId={stack.id}
						{active}
					/>
				</div>
			{/each}
		{:else if isCommitting}
			<StackDraft {projectId} />
		{/if}

		{#if $mode !== 'vertical'}
			<Scrollbar whenToShow="hover" viewport={lanesContentEl} horz />
		{/if}
	</div>
</div>

<style lang="postcss">
	.lanes {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
	}

	.lanes-header {
		display: flex;
		justify-content: space-between;
		gap: 10px;
		align-items: center;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		height: 44px;
		padding-left: 12px;

		& .title {
			flex: 1;
			display: flex;
			align-items: center;
			gap: 6px;
		}

		& .actions {
			display: flex;
		}
	}

	.lanes-content {
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

	.lane {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		scroll-snap-align: start;
		border-right: 1px solid var(--clr-border-2);
		overflow-x: hidden;
		overflow-y: auto;

		&:first-child {
			border-left: 1px solid var(--clr-border-2);
		}

		&.single {
			flex-basis: calc(100% - 30px);
		}
		&.multi {
			flex-shrink: unset;
			flex-basis: 100%;
			min-width: 340px;
		}
		&.vertical {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.pagination-container {
		z-index: var(--z-floating);
		position: absolute;
		display: flex;

		&.horz {
			bottom: 60px;
			right: 6px;
		}

		&.vert {
			bottom: 6px;
			right: 6px;
			transform: rotate(90deg) translateY(100%);
			transform-origin: right bottom;
		}
	}
</style>
