<script lang="ts">
	import BranchFooter from './BranchFooter.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import EmptyStatePlaceholder from '../components/EmptyStatePlaceholder.svelte';
	import InfoMessage from '../components/InfoMessage.svelte';
	import ScrollableContainer from '../components/ScrollableContainer.svelte';
	import PullRequestCard from '../pr/PullRequestCard.svelte';
	import { PromptService } from '$lib/ai/promptService';
	import { AIService } from '$lib/ai/service';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { Project } from '$lib/backend/projects';
	import CommitDialog from '$lib/commit/CommitDialog.svelte';
	import CommitList from '$lib/commit/CommitList.svelte';
	import BranchCardDropzones from '$lib/components/BranchCard/Dropzones.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import { projectAiGenEnabled } from '$lib/config/config';
	import BranchFiles from '$lib/file/BranchFiles.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { User } from '$lib/stores/user';
	import { getContext, getContextStore, getContextStoreBySymbol } from '$lib/utils/context';
	import { computeAddedRemovedByFiles } from '$lib/utils/metrics';
	import { BranchController } from '$lib/vbranches/branchController';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { Branch } from '$lib/vbranches/types';
	import lscache from 'lscache';
	import { onMount } from 'svelte';
	import type { Persisted } from '$lib/persisted/persisted';
	import type { Writable } from 'svelte/store';

	export let isUnapplied = false;
	export let isLaneCollapsed: Persisted<boolean>;
	export let commitBoxOpen: Writable<boolean>;

	const branchController = getContext(BranchController);
	const fileIdSelection = getContext(FileIdSelection);
	const branchStore = getContextStore(Branch);
	const project = getContext(Project);
	const user = getContextStore(User);

	$: branch = $branchStore;

	const aiGenEnabled = projectAiGenEnabled(project.id);
	const aiGenAutoBranchNamingEnabled = projectAiGenAutoBranchNamingEnabled(project.id);

	const aiService = getContext(AIService);
	const promptService = getContext(PromptService);

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const defaultBranchWidthRem = persisted<number>(24, 'defaulBranchWidth' + project.id);
	const laneWidthKey = 'laneWidth_';
	const newVbranchNameRegex = /^virtual\sbranch\s*[\d]*$/;

	let laneWidth: number;

	let scrollViewport: HTMLElement;
	let rsViewport: HTMLElement;

	$: if ($commitBoxOpen && branch.files.length === 0) {
		$commitBoxOpen = false;
	}

	async function generateBranchName() {
		if (!aiGenEnabled) return;

		const hunks = branch.files.flatMap((f) => f.hunks);

		try {
			const prompt = promptService.selectedBranchPrompt(project.id);
			const message = await aiService.summarizeBranch({
				hunks,
				userToken: $user?.access_token,
				branchTemplate: prompt
			});

			if (message && message !== branch.name) {
				branch.name = message;
				branchController.updateBranchName(branch.id, branch.name);
			}
		} catch (e) {
			console.error(e);
			showError('Failed to generate branch name', e);
		}
	}

	$: linesTouched = computeAddedRemovedByFiles(...branch.files);
	$: if (
		$aiGenAutoBranchNamingEnabled &&
		newVbranchNameRegex.test(branch.name.toLowerCase()) &&
		linesTouched.added + linesTouched.removed > 4
	) {
		generateBranchName();
	}

	onMount(() => {
		laneWidth = lscache.get(laneWidthKey + branch.id);
	});
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-container">
		<BranchHeader
			{isUnapplied}
			uncommittedChanges={branch.files.length}
			bind:isLaneCollapsed
			on:action={(e) => {
				if (e.detail === 'generate-branch-name') {
					generateBranchName();
				}
			}}
		/>
	</div>
{:else}
	<div class="resizer-wrapper" bind:this={scrollViewport}>
		<div
			class="branch-card hide-native-scrollbar"
			data-tauri-drag-region
			class:target-branch={branch.active && branch.selectedForChanges}
		>
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
					<BranchHeader
						{isUnapplied}
						bind:isLaneCollapsed
						on:action={(e) => {
							if (e.detail === 'generate-branch-name') {
								generateBranchName();
							}
							if (e.detail === 'collapse') {
								$isLaneCollapsed = true;
							}
						}}
					/>
					<PullRequestCard />

					<div class="card">
						<BranchCardDropzones>
							{#if branch.files?.length > 0}
								<div class="branch-card__files">
									<BranchFiles
										files={branch.files}
										{isUnapplied}
										showCheckboxes={$commitBoxOpen}
										allowMultiple
									/>
									{#if branch.active && branch.conflicted}
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

									{#if branch.active}
										<CommitDialog
											projectId={project.id}
											expanded={commitBoxOpen}
											hasSectionsAfter={branch.commits.length > 0}
											on:action={(e) => {
												if (e.detail === 'generate-branch-name') {
													generateBranchName();
												}
											}}
										/>
									{/if}
								</div>
							{:else if branch.commits.length === 0}
								<div class="new-branch">
									<EmptyStatePlaceholder image={laneNewSvg} width="11rem">
										<svelte:fragment slot="title">This is a new branch</svelte:fragment>
										<svelte:fragment slot="caption">
											You can drag and drop files or parts of files here.
										</svelte:fragment>
									</EmptyStatePlaceholder>
								</div>
							{:else}
								<div class="no-changes" data-dnd-ignore>
									<EmptyStatePlaceholder image={noChangesSvg} width="11rem" hasBottomMargin={false}>
										<svelte:fragment slot="caption"
											>No uncommitted changes on this branch</svelte:fragment
										>
									</EmptyStatePlaceholder>
								</div>
							{/if}
						</BranchCardDropzones>

						<div class="card-commits">
							<CommitList {isUnapplied} />
							<BranchFooter {isUnapplied} />
						</div>
					</div>
				</div>
			</ScrollableContainer>
			<div class="divider-line">
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

	.card {
		flex: 1;
		/* overflow: hidden; */
		/* border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m); */
	}

	.branch-card__files {
		display: flex;
		flex-direction: column;
		flex: 1;
		height: 100%;
		/* border-left: 1px solid var(--clr-border-2);
		border-right: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m) var(--radius-m) 0 0; */
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

	.branch-card :global(.contents) {
		display: flex;
		flex-direction: column;
		min-height: 100%;
	}

	/* COLLAPSED LANE */
	.collapsed-lane-container {
		display: flex;
		flex-direction: column;
		padding: 12px;
		height: 100%;
		border-right: 1px solid var(--clr-border-2);
	}
</style>
