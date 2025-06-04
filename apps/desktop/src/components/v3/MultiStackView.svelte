<script lang="ts">
	import Scrollbar from '$components/Scrollbar.svelte';
	import BranchLayoutMode from '$components/v3/BranchLayoutMode.svelte';
	import BranchList from '$components/v3/BranchList.svelte';
	import MultiStackCreateNew from '$components/v3/MultiStackCreateNew.svelte';
	import MultiStackOfflaneDropzone from '$components/v3/MultiStackOfflaneDropzone.svelte';
	import MultiStackPagination, { scrollToLane } from '$components/v3/MultiStackPagination.svelte';
	import StackDraft from '$components/v3/StackDraft.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import noAssignmentsSvg from '$lib/assets/empty-state/no-assignments.svg?raw';
	import noBranchesSvg from '$lib/assets/empty-state/no-branches.svg?raw';
	import { stackLayoutMode } from '$lib/config/uiFeatureFlags';
	import { UncommittedService } from '$lib/selection/uncommittedService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import type { SelectionId } from '$lib/selection/key';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		selectedId?: string;
		stacks: Stack[];
		active: boolean;
		selectionId: SelectionId;
	};

	const { projectId, selectedId, stacks, active, selectionId }: Props = $props();

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

	const [uiState, uncommittedService] = inject(UiState, UncommittedService);
	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const SHOW_PAGINATION_THRESHOLD = 1;

	let dropzoneActivated = $state(false);
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
						>
							{#snippet assignments()}
								{@const changes = uncommittedService.changesByStackId(stack.id || null)}
								<!-- {#if changes.current.length > 0 || dropzoneActivated} -->
								<!-- class:hidden={changes.current.length === 0 && !dropzoneActivated} -->
								<div
									class="assignments"
									class:assignments__empty={changes.current.length === 0}
									class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
								>
									<WorktreeChanges
										title="Assigned"
										{projectId}
										stackId={stack.id}
										mode="assigned"
										active={selectionId.type === 'worktree' && selectionId.stackId === stack.id}
										onDropzoneActivated={(activated) => {
											dropzoneActivated = activated;
										}}
									>
										{#snippet emptyPlaceholder()}
											<div class="assigned-changes-empty">
												<div class="assigned-changes-empty__svg-wrapper">
													<div class="assigned-changes-empty__svg">
														{@html noAssignmentsSvg}
													</div>
												</div>
												<p class="text-12 text-body assigned-changes-empty__text">
													<!-- <h4 class="text-14 text-semibold">Assign changes</h4> -->
													<!-- <p class="text-12 text-body assigned-changes-empty__description"> -->
													Drop files to assign to the lane
												</p>
											</div>
										{/snippet}
									</WorktreeChanges>
								</div>
								<!-- {/if} -->
							{/snippet}
						</BranchList>
					</div>
				{/each}

				<MultiStackOfflaneDropzone {projectId} />

				{#if lanesSrollableEl && $stackLayoutMode !== 'single'}
					<Scrollbar viewport={lanesSrollableEl} horz={$stackLayoutMode !== 'vertical'} />
				{/if}
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
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

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

	.lane {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		overflow-x: hidden;
		overflow-y: auto;
		border-right: 1px solid var(--clr-border-2);
		scroll-snap-align: start;

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
			max-width: var(--lane-multi-max-width);
		}
		&.vertical {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.pagination-container {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;

		&.horz {
			right: 6px;
			bottom: 60px;
		}

		&.vert {
			right: 6px;
			bottom: 6px;
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

	/* EMPTY ASSIGN AREA */
	.assigned-changes-empty {
		display: flex;
		position: relative;
		padding: 6px 8px 8px;
		overflow: hidden;
		gap: 12px;
		transition: padding 0.12s ease;
	}

	.assigned-changes-empty__svg-wrapper {
		display: flex;
		position: relative;
		flex-shrink: 0;
		width: 70px;
	}

	.assigned-changes-empty__svg {
		position: absolute;
		top: 50%;
		left: 50%;
		width: 100%;
		transform: translate(-50%, -50%);
		transition: top 0.12s ease;
	}

	.assigned-changes-empty__text {
		color: var(--clr-text-2);
		opacity: 0.8;
	}

	.assignments {
		display: flex;
		flex-direction: column;

		margin-bottom: 8px;
		overflow: hidden;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);

		&.dropzone-activated {
			& .assigned-changes-empty {
				padding: 16px 8px 20px;
				background-color: var(--clr-bg-1);
				will-change: padding;
			}

			& .assigned-changes-empty__svg {
				top: 20%;
				will-change: top;
			}
		}
	}

	.assignments__empty {
		margin-top: -12px;
		border-top: none;
		border-top-right-radius: 0;
		border-top-left-radius: 0;
		background-color: var(--clr-bg-2);
	}
</style>
