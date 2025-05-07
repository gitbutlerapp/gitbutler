<script lang="ts">
	import BranchLayoutMode, { type Layout } from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
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

	let lanesEl = $state<HTMLElement>();
	let scrollbar = $state<Scrollbar>();

	let lanesElArray = $state<HTMLElement[]>([]);
	let invisibleLanes = $state<string[]>([]);
	let visibleIndex = $state<number>(0);

	function onScroll() {
		if (stacks.length < 1 && $mode !== 'single') return;

		const scrollLeft = lanesEl?.scrollLeft ?? 0;
		const laneWidth = lanesEl?.offsetWidth ?? 1; // fallback to 1 to avoid divide-by-zero

		const index = Math.round(scrollLeft / laneWidth);

		if (index !== visibleIndex) {
			visibleIndex = index;
		}
	}

	$effect(() => {
		// Explicit scrollbar track size update since changing scroll width
		// does not trigger the resize observer, and changing css does not
		// trigger the mutation observer
		if ($mode) scrollbar?.updateTrack();
	});

	const [uiState] = inject(UiState);
	const drawer = $derived(uiState.project(projectId).drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const SHOW_PAGINATION_THRESHOLD = 1;

	// $effect(() => {
	// 	console.log('visibleLanes', visibleLanes);
	// });
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
		bind:this={lanesEl}
		class:multi={$mode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
		class:single={$mode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
		class:vertical={$mode === 'vertical'}
	>
		{#if invisibleLanes.length >= SHOW_PAGINATION_THRESHOLD}
			<div
				class="pagination-container"
				class:horz={$mode !== 'vertical'}
				class:vert={$mode === 'vertical'}
			>
				<MultiStackPagination
					length={lanesElArray.length}
					activeIndex={$mode === 'single' ? visibleIndex : undefined}
					selectedBranchIndex={stacks.findIndex((s) => {
						return s.id === selectedId;
					})}
					onclick={(index) => scrollToLane(lanesEl, index)}
				/>
			</div>
		{/if}

		{#if stacks.length > 0}
			{#each stacks as stack, i}
				<div
					class="lane"
					class:multi={$mode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
					class:single={$mode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
					class:vertical={$mode === 'vertical'}
					data-id={stack.id}
					bind:this={lanesElArray[i]}
					use:intersectionObserver={{
						callback: (entry) => {
							if (entry?.isIntersecting) {
								invisibleLanes = invisibleLanes.filter((id) => id !== stack.id);
							} else {
								invisibleLanes = [...invisibleLanes, stack.id];
							}
						},
						options: {
							root: lanesEl,
							threshold: 0.5
						}
					}}
				>
					<!-- {stack.id} -->
					<BranchList isVerticalMode={$mode === 'vertical'} {projectId} stackId={stack.id} />
				</div>
			{/each}
		{:else if isCommitting}
			<StackDraft {projectId} />
		{/if}

		{#if $mode !== 'vertical'}
			<Scrollbar whenToShow="hover" viewport={lanesEl} horz onscroll={onScroll} />
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
