<script lang="ts">
	import SeriesList from './SeriesList.svelte';
	import UncommittedChanges from './UncommittedChanges.svelte';
	import CollapsedLane from '$components/CollapsedLane.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Dropzones from '$components/Dropzones.svelte';
	import Resizer from '$components/Resizer.svelte';
	import StackHeader from '$components/v3/StackHeader.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { BranchStack } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { DetailedCommit } from '$lib/commits/commit';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { Project } from '$lib/project/project';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import { type Writable } from 'svelte/store';

	const {
		isLaneCollapsed,
		commitBoxOpen
	}: { isLaneCollapsed: Writable<boolean>; commitBoxOpen: Writable<boolean> } = $props();

	const branchController = getContext(BranchController);
	const fileIdSelection = getContext(FileIdSelection);
	const branchStore = getContextStore(BranchStack);
	const project = getContext(Project);
	const listingService = getForgeListingService();
	const stack = $derived($branchStore);
	const stackPublishingService = getContext(StackPublishingService);

	const width = persistWithExpiration<number>(24, 'stackWidth_' + project.id, 7 * 1440);
	let lastPush = $state<Date | undefined>();

	let rsViewport = $state<HTMLElement>();

	const branchHasFiles = $derived(stack.files !== undefined && stack.files.length > 0);
	const branchHasNoCommits = $derived(stack.validSeries.flatMap((s) => s.patches).length === 0);

	$effect(() => {
		if ($commitBoxOpen && stack.files.length === 0) {
			commitBoxOpen.set(false);
		}
	});

	let scrollEndVisible = $state(true);
	let isPushingCommits = $state(false);

	const { upstreamPatches, branchPatches, hasConflicts } = $derived.by(() => {
		let hasConflicts = false;
		const upstreamPatches: DetailedCommit[] = [];
		const branchPatches: DetailedCommit[] = [];

		stack.validSeries.map((series) => {
			upstreamPatches.push(...series.upstreamPatches);
			branchPatches.push(...series.patches);
			hasConflicts = branchPatches.some((patch) => patch.conflicted);
		});

		return {
			upstreamPatches,
			branchPatches,
			hasConflicts
		};
	});

	const canPush = $derived.by(() => {
		if (upstreamPatches.filter((p) => !p.isIntegrated).length > 0) {
			return true;
		}
		if (branchPatches.some((p) => p.status === 'LocalOnly' || p.status === 'Remote')) {
			return true;
		}
		return false;
	});

	async function push() {
		isPushingCommits = true;
		try {
			await branchController.pushBranch(stack.id, stack.requiresForce);
			$listingService?.refresh();
			lastPush = new Date();
			await pushButlerReviewStacks();
		} finally {
			isPushingCommits = false;
		}
	}

	async function pushButlerReviewStacks() {
		const topPushableBranch = stack.validSeries.find((series) => series.reviewId);
		if (!topPushableBranch) return;

		await stackPublishingService.upsertStack(stack.id, topPushableBranch.name);
	}
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-container">
		<CollapsedLane uncommittedChanges={stack.files.length} {isLaneCollapsed} />
		<div class="collapsed-lane-divider" data-remove-from-draggable></div>
	</div>
{:else}
	<div class="resizer-wrapper">
		<div class="branch-card hide-native-scrollbar" class:target-branch={stack.selectedForChanges}>
			<ScrollableContainer
				wide
				padding={{
					top: 12,
					bottom: 12
				}}
			>
				<div bind:this={rsViewport} style:width={`${$width}rem`} class="branch-card__contents">
					<StackHeader
						{stack}
						onCollapseButtonClick={() => {
							$isLaneCollapsed = true;
						}}
					/>
					<div class="card-stacking">
						{#if branchHasFiles}
							<UncommittedChanges {commitBoxOpen} />
						{:else if branchHasNoCommits}
							<Dropzones type="file">
								<div class="new-branch">
									<EmptyStatePlaceholder image={laneNewSvg} width={180} bottomMargin={48}>
										{#snippet title()}
											This is a new lane
										{/snippet}
										{#snippet caption()}
											You can drag and drop files<br />or parts of files here.
										{/snippet}
									</EmptyStatePlaceholder>
								</div>
							</Dropzones>
						{:else}
							<Dropzones type="file">
								<div class="no-changes">
									<EmptyStatePlaceholder image={noChangesSvg} width={180}>
										{#snippet caption()}
											No uncommitted<br />changes on this lane
										{/snippet}
									</EmptyStatePlaceholder>
								</div>
							</Dropzones>
						{/if}
						<Spacer dotted />
						<div style:position="relative">
							<div class="lane-branches">
								<SeriesList {stack} {lastPush} />
							</div>
							{#if canPush}
								<div
									class="lane-branches__action"
									class:scroll-end-visible={scrollEndVisible}
									use:intersectionObserver={{
										callback: (entry) => {
											if (entry?.isIntersecting) {
												scrollEndVisible = false;
											} else {
												scrollEndVisible = true;
											}
										},
										options: {
											root: null,
											rootMargin: `-100% 0px 0px 0px`,
											threshold: 0
										}
									}}
								>
									<Button
										style="neutral"
										wide
										loading={isPushingCommits}
										disabled={hasConflicts}
										tooltip={hasConflicts
											? 'In order to push, please resolve any conflicted commits.'
											: undefined}
										onclick={push}
									>
										{stack.requiresForce
											? 'Force push'
											: stack.validSeries.length > 1
												? 'Push All'
												: 'Push'}
									</Button>
								</div>
							{/if}
						</div>
					</div>
				</div>
			</ScrollableContainer>
			<div class="divider-line">
				{#if rsViewport}
					<Resizer
						viewport={rsViewport}
						direction="right"
						minWidth={25}
						sticky
						defaultLineColor={$fileIdSelection.length === 1 ? 'transparent' : 'var(--clr-border-2)'}
						onWidth={(value) => ($width = value)}
					/>
				{/if}
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.resizer-wrapper {
		position: relative;
		display: flex;
		height: 100%;
	}

	.branch-card {
		height: 100%;
		position: relative;
		user-select: none;
		overflow-x: hidden;
		overflow-y: scroll;
	}

	.lane-branches {
		display: flex;
		flex-direction: column;
	}

	.lane-branches__action {
		z-index: var(--z-lifted);
		position: sticky;
		padding: 0 12px 12px;
		margin: 0 -12px 1px -12px;
		bottom: 0;
		transition: background-color var(--transition-fast);

		&:global(.merge-all > button:not(:last-child)) {
			margin-bottom: 8px;
		}

		&:after {
			content: '';
			display: block;
			position: absolute;
			bottom: 0;
			left: 0;
			height: calc(100% + 12px);
			width: 100%;
			z-index: -1;
			background-color: var(--clr-bg-1);
			border-top: 1px solid var(--clr-border-2);

			transform: translateY(0);
			opacity: 0;
			transition: opacity var(--transition-fast);
		}

		&:not(.scroll-end-visible):after {
			opacity: 1;
		}
	}

	.divider-line {
		z-index: var(--z-lifted);
		position: absolute;
		top: 0;
		right: 0;
		height: 100%;
	}

	.branch-card__contents {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 100%;
		padding: 12px 12px 0;
	}

	.card-stacking {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.no-changes,
	.new-branch {
		border-radius: 0 0 var(--radius-m) var(--radius-m) !important;
		border: 1px solid var(--clr-border-2);
		border-top-width: 0;
		background: var(--clr-bg-1);
	}

	.new-branch,
	.no-changes {
		flex-grow: 1;
		user-select: none;
		display: flex;
		height: 100%;
		flex-direction: column;
		align-items: center;
		color: var(--clr-scale-ntrl-60);
		justify-content: center;
		cursor: default; /* was defaulting to text cursor */
		border-top-width: 0px;
	}

	/* COLLAPSED LANE */
	.collapsed-lane-container {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 12px;
		height: 100%;
	}

	.collapsed-lane-divider {
		position: absolute;
		top: 0;
		right: 0;
		width: 1px;
		height: 100%;
		background-color: var(--clr-border-2);
	}
</style>
