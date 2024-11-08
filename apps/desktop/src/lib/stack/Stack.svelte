<script lang="ts">
	import SeriesList from './SeriesList.svelte';
	import StackHeader from './header/StackHeader.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { Project } from '$lib/backend/projects';
	import Dropzones from '$lib/branch/Dropzones.svelte';
	import CommitDialog from '$lib/commit/CommitDialog.svelte';
	import BranchFiles from '$lib/file/BranchFiles.svelte';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import CollapsedLane from '$lib/stack/CollapsedLane.svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { BranchController } from '$lib/vbranches/branchController';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { DetailedCommit, BranchStack } from '$lib/vbranches/types';
	import { getContext, getContextStore, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import EmptyStatePlaceholder from '@gitbutler/ui/EmptyStatePlaceholder.svelte';
	import Spacer from '@gitbutler/ui/Spacer.svelte';
	import lscache from 'lscache';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const {
		isLaneCollapsed,
		commitBoxOpen
	}: { isLaneCollapsed: Writable<boolean>; commitBoxOpen: Writable<boolean> } = $props();

	const branchController = getContext(BranchController);
	const fileIdSelection = getContext(FileIdSelection);
	const stackStore = getContextStore(BranchStack);
	const project = getContext(Project);
	const stack = $derived($stackStore);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const defaultBranchWidthRem = persisted<number>(24, 'defaulBranchWidth' + project.id);
	const laneWidthKey = 'laneWidth_';

	let laneWidth: number | undefined = $state();

	let commitDialog = $state<CommitDialog>();
	let rsViewport = $state<HTMLElement>();

	$effect(() => {
		if ($commitBoxOpen && stack.files.length === 0) {
			commitBoxOpen.set(false);
		}
	});

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey + stack.id);
	});

	let scrollEndVisible = $state(true);
	let isPushingCommits = $state(false);

	const { upstreamPatches, branchPatches, hasConflicts } = $derived.by(() => {
		let hasConflicts = false;
		const upstreamPatches: DetailedCommit[] = [];
		const branchPatches: DetailedCommit[] = [];

		stack.series.map((series) => {
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

	let canPush = $derived.by(() => {
		if (upstreamPatches.length > 0) return true;
		if (branchPatches.some((p) => !['localAndRemote', 'integrated'].includes(p.status)))
			return true;
		if (branchPatches.some((p) => p.status !== 'integrated' && p.remoteCommitId !== p.id))
			return true;

		return false;
	});

	const listingService = getForgeListingService();

	async function push() {
		isPushingCommits = true;
		try {
			await branchController.pushBranch(stack.id, stack.requiresForce, true);
			$listingService?.refresh();
			// TODO: Refresh prMonitor and checksMonitor upon push
		} finally {
			isPushingCommits = false;
		}
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
				<div
					bind:this={rsViewport}
					style:width={`${laneWidth || $defaultBranchWidthRem}rem`}
					class="branch-card__contents"
					data-tauri-drag-region
				>
					<StackHeader
						branch={stack}
						onCollapseButtonClick={() => {
							$isLaneCollapsed = true;
						}}
					/>
					<div class="card-stacking">
						{#if stack.files?.length > 0}
							<div class="branch-card__files">
								<Dropzones type="file">
									<BranchFiles
										isUnapplied={false}
										files={stack.files}
										showCheckboxes={$commitBoxOpen}
										allowMultiple
										commitDialogExpanded={commitBoxOpen}
										focusCommitDialog={() => commitDialog?.focus()}
									/>
									{#if stack.conflicted}
										<div class="card-notifications">
											<InfoMessage filled outlined={false} style="error">
												<svelte:fragment slot="title">
													{#if stack.files.some((f) => f.conflicted)}
														This virtual branch conflicts with upstream changes. Please resolve all
														conflicts and commit before you can continue.
													{:else}
														Please commit your resolved conflicts to continue.
													{/if}
												</svelte:fragment>
											</InfoMessage>
										</div>
									{/if}
								</Dropzones>

								<CommitDialog
									bind:this={commitDialog}
									projectId={project.id}
									expanded={commitBoxOpen}
									hasSectionsAfter={stack.commits.length > 0}
								/>
							</div>
						{:else if stack.commits.length === 0}
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
						<div class="lane-branches">
							<SeriesList branch={stack} />
						</div>
					</div>
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
							{stack.requiresForce ? 'Force push' : stack.series.length > 1 ? 'Push All' : 'Push'}
						</Button>
					</div>
				{/if}
			</ScrollableContainer>
			<div class="divider-line">
				{#if rsViewport}
					<Resizer
						viewport={rsViewport}
						direction="right"
						minWidth={380}
						sticky
						defaultLineColor={$fileIdSelection.length === 1 ? 'transparent' : 'var(--clr-border-2)'}
						on:width={(e) => {
							laneWidth = e.detail / (16 * $userSettings.zoom);
							lscache.set(laneWidthKey + stack.id, laneWidth, 7 * 1440); // 7 day ttl
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
		position: relative;
		z-index: var(--z-lifted);
		position: sticky;
		padding: 0 12px 12px;
		margin-bottom: 1px;
		bottom: 0;
		transition: background-color var(--transition-fast);

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

	.branch-card__files,
	.no-changes,
	.new-branch {
		border-radius: 0 0 var(--radius-m) var(--radius-m) !important;
		border: 1px solid var(--clr-border-2);
		border-top-width: 0;
		background: var(--clr-bg-1);
	}

	.branch-card__files {
		display: flex;
		flex-direction: column;
		flex: 1;
		height: 100%;
	}

	.card-notifications {
		display: flex;
		flex-direction: column;
		padding: 12px;
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
