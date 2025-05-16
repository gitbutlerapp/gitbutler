<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import BranchLayoutMode from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { stackLayoutMode } from '$lib/config/uiFeatureFlags';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
		active: boolean;
	};

	const { projectId, selectedId, stacks, active }: Props = $props();

	let lanesSrollableEl = $state<HTMLDivElement>();
	let lanesScrollableWidth = $state<number>(0);
	let lanesScrollableHeight = $state<number>(0);
	let scrollbar = $state<Scrollbar>();

	let laneWidths = $state<number[]>([]);
	let lineHights = $state<number[]>([]);
	let isNotEnoughHorzSpace = $derived(
		(lanesScrollableWidth ?? 0) < (laneWidths.length - 1) * (laneWidths[0] ?? 0)
	);
	let isNotEnoughVertSpace = $derived.by(() => {
		const shortenArray = lineHights.slice(0, lineHights.length - 1);
		return lanesScrollableHeight < shortenArray.reduce((acc, height) => acc + height, 0);
	});
	let visibleIndexes = $state<number[]>([0]);

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

<div class="lanes">
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

	{#if isNotEnoughHorzSpace && isNotEnoughVertSpace}
		<div
			class="pagination-container"
			class:horz={$stackLayoutMode !== 'vertical'}
			class:vert={$stackLayoutMode === 'vertical'}
		>
			<MultiStackPagination
				length={stacks.length}
				{visibleIndexes}
				selectedBranchIndex={stacks.findIndex((s) => {
					return s.id === selectedId;
				})}
				onclick={(index) =>
					scrollToLane(lanesSrollableEl, index, $stackLayoutMode === 'vertical' ? 'vert' : 'horz')}
			/>
		</div>
	{/if}

	<div class="lanes-viewport">
		<div
			class="lanes-scrollable hide-native-scrollbar dotted-pattern"
			bind:this={lanesSrollableEl}
			bind:clientWidth={lanesScrollableWidth}
			bind:clientHeight={lanesScrollableHeight}
			class:multi={$stackLayoutMode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
			class:single={$stackLayoutMode === 'single' && stacks.length >= SHOW_PAGINATION_THRESHOLD}
			class:vertical={$stackLayoutMode === 'vertical'}
		>
			{#if lanesSrollableEl}
				<Scrollbar viewport={lanesSrollableEl} horz={$stackLayoutMode !== 'vertical'} />
			{/if}

			{#if stacks.length > 0}
				{#each stacks as stack, i}
					<div
						class="lane"
						class:multi={$stackLayoutMode === 'multi' || stacks.length < SHOW_PAGINATION_THRESHOLD}
						class:single={$stackLayoutMode === 'single' &&
							stacks.length >= SHOW_PAGINATION_THRESHOLD}
						class:single-fullwidth={$stackLayoutMode === 'single' && stacks.length === 1}
						class:vertical={$stackLayoutMode === 'vertical'}
						data-id={stack.id}
						bind:clientWidth={laneWidths[i]}
						bind:clientHeight={lineHights[i]}
						data-testid={TestId.Stack}
						data-testid-stackid={stack.id}
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
								root: lanesSrollableEl
							}
						}}
					>
						<BranchList
							isVerticalMode={$stackLayoutMode === 'vertical'}
							{projectId}
							stackId={stack.id}
							{active}
						/>
					</div>
				{/each}

				<MultiStackOfflaneDropzone {projectId} />
			{:else if isCommitting}
				<StackDraft {projectId} />
			{:else}
				<div class="no-stacks-placeholder">
					<EmptyStatePlaceholder image={noBranchesSvg} bottomMargin={48}>
						{#snippet title()}
							You have no branches
						{/snippet}
						{#snippet caption()}
							Create a new branch for<br />a feature, fix, or idea!
						{/snippet}
					</EmptyStatePlaceholder>
				</div>
			{/if}
		</div>
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
			overflow: hidden;
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
		position: relative;
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
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
		&.single-fullwidth {
			flex-basis: 100%;
		}
		&.multi {
			width: 100%;
			max-width: 340px;
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

	.no-stacks-placeholder {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 100%;
		height: 100%;
		display: flex;
		flex-direction: column;
		overflow: hidden;

		&:after {
			z-index: -1;
			content: '';
			width: 600px;
			height: 600px;
			position: absolute;
			top: calc(50% - 50px);
			left: 50%;
			transform: translate(-50%, -50%);
			border-radius: 100%;
			background: radial-gradient(var(--clr-bg-2) 0%, oklch(from var(--clr-bg-2) l c h / 0) 70%);
		}
	}
</style>
