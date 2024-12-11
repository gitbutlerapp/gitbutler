<script lang="ts">
	import SeriesList from './SeriesList.svelte';
	import UncommittedChanges from './UncommittedChanges.svelte';
	import StackHeader from './header/StackHeader.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import Dropzones from '$lib/branch/Dropzones.svelte';
	import { getForge } from '$lib/forge/interface/forge';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { type MergeMethod } from '$lib/forge/interface/types';
	import { showError } from '$lib/notifications/toasts';
	import MergeButton from '$lib/pr/MergeButton.svelte';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import CollapsedLane from '$lib/stack/CollapsedLane.svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { DetailedCommit, VirtualBranch } from '$lib/vbranches/types';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext, getContextStore, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import lscache from 'lscache';
	import { onMount } from 'svelte';
	import { type Writable } from 'svelte/store';

	const {
		isLaneCollapsed,
		commitBoxOpen
	}: { isLaneCollapsed: Writable<boolean>; commitBoxOpen: Writable<boolean> } = $props();

	const vbranchService = getContext(VirtualBranchService);
	const branchController = getContext(BranchController);
	const fileIdSelection = getContext(FileIdSelection);
	const branchStore = getContextStore(VirtualBranch);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const prService = getForgePrService();
	const listingService = getForgeListingService();
	const branch = $derived($branchStore);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const defaultBranchWidthRem = persisted<number>(24, 'defaulBranchWidth' + project.id);
	let lastPush = $state<Date | undefined>();

	const laneWidthKey = 'laneWidth_';
	let laneWidth: number | undefined = $state();
	let rsViewport = $state<HTMLElement>();

	const branchHasFiles = $derived(branch.files !== undefined && branch.files.length > 0);
	const branchHasNoCommits = $derived(branch.validSeries.flatMap((s) => s.patches).length === 0);

	$effect(() => {
		if ($commitBoxOpen && branch.files.length === 0) {
			commitBoxOpen.set(false);
		}
	});

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey + branch.id);
	});

	let scrollEndVisible = $state(true);
	let isPushingCommits = $state(false);
	let isMergingSeries = $state(false);

	const { upstreamPatches, branchPatches, hasConflicts } = $derived.by(() => {
		let hasConflicts = false;
		const upstreamPatches: DetailedCommit[] = [];
		const branchPatches: DetailedCommit[] = [];

		branch.validSeries.map((series) => {
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
		if (upstreamPatches.filter((p) => !p.isIntegrated).length > 0) return true;
		if (branchPatches.some((p) => !['localAndRemote', 'integrated'].includes(p.status)))
			return true;
		return false;
	});

	async function push() {
		isPushingCommits = true;
		try {
			await branchController.pushBranch(branch.id, branch.requiresForce);
			$listingService?.refresh();
			lastPush = new Date();
		} finally {
			isPushingCommits = false;
		}
	}

	async function checkMergeable() {
		const nonArchivedBranches = branch.validSeries.filter((s) => !s.archived);
		if (nonArchivedBranches.length <= 1) return false;

		const seriesMergeResponse = await Promise.allSettled(
			nonArchivedBranches.map((series) => {
				if (!series.prNumber) return Promise.reject();

				const detailedPr = $prService?.get(series.prNumber);
				return detailedPr;
			})
		);

		return seriesMergeResponse.every((s) => {
			if (s.status === 'fulfilled' && s.value) {
				return s.value.mergeable === true;
			}
			return false;
		});
	}

	// Create monitor on top series in order for us to trigger mergeabilitiy test once its
	// checks have completed. Using the top branch as it's checks are most likely to have been
	// started last and therefore complete last.
	const forge = getForge();
	const checksMonitor = $derived(
		$forge?.checksMonitor(branch.validSeries.filter((s) => !s.archived)[0]?.name ?? '')
	);
	const checks = $derived(checksMonitor?.status);

	let canMergeAll = $derived.by(() => {
		// Force this to rerun once the checks have completed and we can check mergeability again
		void $checks;
		return checkMergeable();
	});

	async function mergeAll(method: MergeMethod) {
		isMergingSeries = true;
		try {
			const topBranch = branch.validSeries[0];

			if (topBranch?.prNumber && $prService) {
				const targetBase = $baseBranch.branchName.replace(`${$baseBranch.remoteName}/`, '');
				await $prService.update(topBranch.prNumber, { targetBase });
				await $prService.merge(method, topBranch.prNumber);
				await baseBranchService.fetchFromRemotes();
				toasts.success('Stack Merged Successfully');

				await Promise.all([
					$prService?.prMonitor(topBranch.prNumber).refresh(),
					$listingService?.refresh(),
					vbranchService.refresh(),
					baseBranchService.refresh()
				]);
			}
		} catch (e) {
			console.error(e);
			showError('Failed to merge PR', e);
		} finally {
			isMergingSeries = false;
		}
	}
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-container">
		<CollapsedLane uncommittedChanges={branch.files.length} {isLaneCollapsed} />
		<div class="collapsed-lane-divider" data-remove-from-draggable></div>
	</div>
{:else}
	<div class="resizer-wrapper">
		<div class="branch-card hide-native-scrollbar" class:target-branch={branch.selectedForChanges}>
			<ScrollableContainer
				wide
				padding={{
					top: 12,
					bottom: 12
				}}
			>
				<div
					bind:this={rsViewport}
					style:width={`${laneWidth || $defaultBranchWidthRem}rem`}
					class="branch-card__contents"
				>
					<StackHeader
						{branch}
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
								<SeriesList {branch} {lastPush} />
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
										kind="solid"
										wide
										loading={isPushingCommits}
										disabled={hasConflicts}
										tooltip={hasConflicts
											? 'In order to push, please resolve any conflicted commits.'
											: undefined}
										onclick={push}
									>
										{branch.requiresForce
											? 'Force push'
											: branch.validSeries.length > 1
												? 'Push All'
												: 'Push'}
									</Button>
								</div>
							{/if}
							{#await canMergeAll then isMergeable}
								{#if isMergeable}
									<div
										class="lane-branches__action merge-all"
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
										<MergeButton
											style="neutral"
											kind="solid"
											wide
											projectId={project.id}
											tooltip="Merge all possible branches"
											loading={isMergingSeries}
											onclick={mergeAll}
										/>
									</div>
								{/if}
							{/await}
						</div>
					</div>
				</div>
			</ScrollableContainer>
			<div class="divider-line">
				{#if rsViewport}
					<Resizer
						viewport={rsViewport}
						direction="right"
						minWidth={380}
						sticky
						defaultLineColor={$fileIdSelection.length === 1 ? 'transparent' : 'var(--clr-border-2)'}
						onWidth={(value) => {
							laneWidth = value / (16 * $userSettings.zoom);
							lscache.set(laneWidthKey + branch.id, laneWidth, 7 * 1440); // 7 day ttl
							$defaultBranchWidthRem = laneWidth;
						}}
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
