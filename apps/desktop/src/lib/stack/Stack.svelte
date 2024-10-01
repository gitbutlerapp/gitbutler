<script lang="ts">
	import StackHeader from './StackHeader.svelte';
	import StackSeries from './StackSeries.svelte';
	import EmptyStatePlaceholder from '../components/EmptyStatePlaceholder.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import Dropzones from '$lib/branch/Dropzones.svelte';
	import StackingNewStackCard from '$lib/branch/StackingNewStackCard.svelte';
	import CommitDialog from '$lib/commit/CommitDialog.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import BranchFiles from '$lib/file/BranchFiles.svelte';
	import { getGitHostChecksMonitor } from '$lib/gitHost/interface/gitHostChecksMonitor';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrMonitor } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { showError } from '$lib/notifications/toasts';
	import { persisted } from '$lib/persisted/persisted';
	import { isFailure } from '$lib/result';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import Resizer from '$lib/shared/Resizer.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { User } from '$lib/stores/user';
	import { getContext, getContextStore, getContextStoreBySymbol } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import {
		// getIntegratedCommits,
		getLocalAndRemoteCommits,
		getLocalCommits
		// getRemoteCommits
	} from '$lib/vbranches/contexts';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import lscache from 'lscache';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';

	const {
		isLaneCollapsed,
		commitBoxOpen
	}: { isLaneCollapsed: Writable<boolean>; commitBoxOpen: Writable<boolean> } = $props();

	const branchController = getContext(BranchController);
	const fileIdSelection = getContext(FileIdSelection);
	const branchStore = getContextStore(VirtualBranch);
	const project = getContext(Project);
	const user = getContextStore(User);
	const baseBranch = getContextStore(BaseBranch);

	const branch = $derived($branchStore);

	const aiGenEnabled = projectAiGenEnabled(project.id);

	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const defaultBranchWidthRem = persisted<number>(24, 'defaulBranchWidth' + project.id);
	const laneWidthKey = 'laneWidth_';

	let laneWidth: number | undefined = $state();

	let commitDialog = $state<CommitDialog>();
	let scrollViewport = $state<HTMLElement>();
	let rsViewport = $state<HTMLElement>();

	$effect(() => {
		if ($commitBoxOpen && branch.files.length === 0) {
			commitBoxOpen.set(false);
		}
	});

	async function generateBranchName() {
		if (!aiGenEnabled) return;

		const hunks = branch.files.flatMap((f) => f.hunks);

		const prompt = promptService.selectedBranchPrompt(project.id);
		const messageResult = await aiService.summarizeBranch({
			hunks,
			userToken: $user?.access_token,
			branchTemplate: prompt
		});

		if (isFailure(messageResult)) {
			console.error(messageResult.failure);
			showError('Failed to generate branch name', messageResult.failure);

			return;
		}

		const message = messageResult.value;

		if (message && message !== branch.name) {
			branch.name = message;
			branchController.updateBranchName(branch.id, branch.name);
		}
	}

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey + branch.id);
	});

	const localCommits = getLocalCommits();
	const localAndRemoteCommits = getLocalAndRemoteCommits();
	// const integratedCommits = getIntegratedCommits();
	// const remoteCommits = getRemoteCommits();

	let isPushingCommits = $state(false);
	const localCommitsConflicted = $derived($localCommits.some((commit) => commit.conflicted));
	const localAndRemoteCommitsConflicted = $derived(
		$localAndRemoteCommits.some((commit) => commit.conflicted)
	);

	const listingService = getGitHostListingService();
	const prMonitor = getGitHostPrMonitor();
	const checksMonitor = getGitHostChecksMonitor();

	async function push() {
		isPushingCommits = true;
		try {
			await branchController.pushBranch(branch.id, branch.requiresForce, true);
			$listingService?.refresh();
			$prMonitor?.refresh();
			$checksMonitor?.update();
		} finally {
			isPushingCommits = false;
		}
	}

	function addSeries(e: MouseEvent) {
		e.stopPropagation();
		console.log('CREATE SERIES.BRANCH', { branch, baseBranchName: $baseBranch.remoteName });
		const topChangeId = branch.commits.at(-1)?.changeId;
		if (topChangeId) {
			branchController.createChangeReference(
				branch?.id || '',
				'refs/remotes/' +
					$baseBranch.remoteName +
					'/' +
					`series-${Math.floor(Math.random() * 1000)}`,
				topChangeId
			);
		}
	}
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-container">
		<StackHeader
			uncommittedChanges={branch.files.length}
			onGenerateBranchName={generateBranchName}
			{isLaneCollapsed}
		/>
		<div class="collapsed-lane-divider" data-remove-from-draggable></div>
	</div>
{:else}
	<div class="resizer-wrapper" bind:this={scrollViewport}>
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
					data-tauri-drag-region
				>
					<StackHeader {isLaneCollapsed} onGenerateBranchName={generateBranchName} />
					<div class="card-stacking">
						{#if branch.files?.length > 0}
							<div class="branch-card__files card">
								<Dropzones>
									<BranchFiles
										isUnapplied={false}
										files={branch.files}
										showCheckboxes={$commitBoxOpen}
										allowMultiple
										commitDialogExpanded={commitBoxOpen}
										focusCommitDialog={() => commitDialog?.focus()}
									/>
									{#if branch.conflicted}
										<div class="card-notifications">
											<InfoMessage filled outlined={false} style="error">
												<svelte:fragment slot="title">
													{#if branch.files.some((f) => f.conflicted)}
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
									hasSectionsAfter={branch.commits.length > 0}
								/>
							</div>
						{:else if branch.commits.length === 0}
							<Dropzones>
								<div class="new-branch card">
									<EmptyStatePlaceholder image={laneNewSvg} width="11rem">
										<svelte:fragment slot="title">This is a new branch</svelte:fragment>
										<svelte:fragment slot="caption">
											You can drag and drop files or parts of files here.
										</svelte:fragment>
									</EmptyStatePlaceholder>
								</div>
							</Dropzones>
						{:else}
							<Dropzones>
								<div class="no-changes card">
									<EmptyStatePlaceholder image={noChangesSvg} width="11rem" hasBottomMargin={false}>
										<svelte:fragment slot="caption">
											No uncommitted changes on this branch
										</svelte:fragment>
									</EmptyStatePlaceholder>
								</div>
							</Dropzones>
						{/if}
						<Spacer dotted />
						<div class="lane-branches">
							<StackingNewStackCard branchId={branch.id} {addSeries} />
							<StackSeries {branch} />
						</div>
						<!-- TODO: Sticky styling -->
						<div class="lane-branches__action">
							<Button
								style="pop"
								kind="solid"
								wide
								loading={isPushingCommits}
								disabled={localCommitsConflicted || localAndRemoteCommitsConflicted}
								tooltip={localCommitsConflicted
									? 'In order to push, please resolve any conflicted commits.'
									: undefined}
								onclick={push}
							>
								{branch.requiresForce ? 'Force push' : 'Push'}
							</Button>
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
						on:width={(e) => {
							laneWidth = e.detail / (16 * $userSettings.zoom);
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

	:global(.lane-branches > *) {
		margin-bottom: 12px;
	}

	.lane-branches__action {
		z-index: var(--z-lifted);
		position: sticky;
		bottom: 0;
		transition:
			background-color 0.3s ease,
			box-shadow 0.3s ease;
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
		padding: 12px;
	}

	.card-stacking {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.branch-card__files.card,
	.no-changes.card,
	.new-branch.card {
		border-radius: 0 0 var(--radius-m) var(--radius-m) !important;
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
