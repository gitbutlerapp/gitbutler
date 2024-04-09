<script lang="ts">
	import BranchCommits from './BranchCommits.svelte';
	import BranchFiles from './BranchFiles.svelte';
	import BranchHeader from './BranchHeader.svelte';
	import CommitDialog from './CommitDialog.svelte';
	import DropzoneOverlay from './DropzoneOverlay.svelte';
	import InfoMessage from './InfoMessage.svelte';
	import PullRequestCard from './PullRequestCard.svelte';
	import ScrollableContainer from './ScrollableContainer.svelte';
	import laneNewSvg from '$lib/assets/empty-state/lane-new.svg?raw';
	import noChangesSvg from '$lib/assets/empty-state/lane-no-changes.svg?raw';
	import { AIService } from '$lib/backend/aiService';
	import { User } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import Resizer from '$lib/components/Resizer.svelte';
	import { projectAiGenAutoBranchNamingEnabled } from '$lib/config/config';
	import { projectAiGenEnabled } from '$lib/config/config';
	import {
		DraggableCommit,
		DraggableFile,
		DraggableHunk,
		DraggableRemoteCommit
	} from '$lib/dragging/draggables';
	import { dropzone } from '$lib/dragging/dropzone';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContext, getContextStore, getContextStoreBySymbol } from '$lib/utils/context';
	import { computeAddedRemovedByFiles } from '$lib/utils/metrics';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { FileIdSelection } from '$lib/vbranches/fileIdSelection';
	import { filesToOwnership } from '$lib/vbranches/ownership';
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
			const message = await aiService.summarizeBranch({
				hunks,
				userToken: $user?.access_token
			});

			if (message && message !== branch.name) {
				branch.name = message;
				branchController.updateBranchName(branch.id, branch.name);
			}
		} catch (e) {
			console.error(e);
			toasts.error('Failed to generate branch name');
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

	function acceptMoveCommit(data: any) {
		return data instanceof DraggableCommit && data.branchId != branch.id && data.isHeadCommit;
	}
	function onCommitDrop(data: DraggableCommit) {
		branchController.moveCommit(branch.id, data.commit.id);
	}

	function acceptCherrypick(data: any) {
		return data instanceof DraggableRemoteCommit && data.branchId == branch.id;
	}

	function onCherrypicked(data: DraggableRemoteCommit) {
		branchController.cherryPick(branch.id, data.remoteCommit.id);
	}

	function acceptBranchDrop(data: any) {
		if (data instanceof DraggableHunk && data.branchId != branch.id) {
			return !data.hunk.locked;
		} else if (data instanceof DraggableFile && data.branchId && data.branchId != branch.id) {
			return !data.files.some((f) => f.locked);
		} else {
			return false;
		}
	}

	function onBranchDrop(data: DraggableHunk | DraggableFile) {
		if (data instanceof DraggableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.updateBranchOwnership(
				branch.id,
				(newOwnership + '\n' + branch.ownership).trim()
			);
		} else if (data instanceof DraggableFile) {
			const newOwnership = filesToOwnership(data.files);
			branchController.updateBranchOwnership(
				branch.id,
				(newOwnership + '\n' + branch.ownership).trim()
			);
		}
	}
</script>

{#if $isLaneCollapsed}
	<div class="collapsed-lane-wrapper">
		<BranchHeader
			{isUnapplied}
			bind:isLaneCollapsed
			on:action={(e) => {
				if (e.detail == 'generate-branch-name') {
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
					top: `var(--size-12)`,
					bottom: `var(--size-12)`
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
							if (e.detail == 'generate-branch-name') {
								generateBranchName();
							}
						}}
					/>
					<PullRequestCard isLaneCollapsed={$isLaneCollapsed} />
					<!-- DROPZONES -->
					<DropzoneOverlay class="cherrypick-dz-marker" label="Apply here" />
					<DropzoneOverlay class="cherrypick-dz-marker" label="Apply here" />
					<DropzoneOverlay class="lane-dz-marker" label="Move here" />

					<div
						class="branch-card__dropzone-wrapper"
						use:dropzone={{
							hover: 'move-commit-dz-hover',
							active: 'move-commit-dz-active',
							accepts: acceptMoveCommit,
							onDrop: onCommitDrop,
							disabled: isUnapplied
						}}
						use:dropzone={{
							hover: 'cherrypick-dz-hover',
							active: 'cherrypick-dz-active',
							accepts: acceptCherrypick,
							onDrop: onCherrypicked,
							disabled: isUnapplied
						}}
						use:dropzone={{
							hover: 'lane-dz-hover',
							active: 'lane-dz-active',
							accepts: acceptBranchDrop,
							onDrop: onBranchDrop,
							disabled: isUnapplied
						}}
					>
						<DropzoneOverlay class="cherrypick-dz-marker" label="Apply here" />
						<DropzoneOverlay class="lane-dz-marker" label="Move here" />
						<DropzoneOverlay class="move-commit-dz-marker" label="Move here" />

						{#if branch.files?.length > 0}
							<div class="card">
								<BranchFiles
									files={branch.files}
									{isUnapplied}
									showCheckboxes={$commitBoxOpen}
									allowMultiple
								/>
								{#if branch.active && branch.conflicted}
									<div class="card-notifications">
										<InfoMessage noRadius filled outlined={false} style="error">
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
										on:action={(e) => {
											if (e.detail == 'generate-branch-name') {
												generateBranchName();
											}
										}}
									/>
								{/if}
							</div>
						{:else if branch.commits.length == 0}
							<div class="new-branch card" data-dnd-ignore>
								<div class="new-branch__content">
									<div class="new-branch__image">
										{@html laneNewSvg}
									</div>
									<h2 class="new-branch__title text-base-body-15 text-semibold">
										This is a new branch.
									</h2>
									<p class="new-branch__caption text-base-body-13">
										You can drag and drop files or parts of files here.
									</p>
								</div>
							</div>
						{:else}
							<!-- attention: these markers have custom css at the bottom of thise file -->
							<div class="no-changes card" data-dnd-ignore>
								<div class="new-branch__content">
									<div class="new-branch__image">
										{@html noChangesSvg}
									</div>
									<h2 class="new-branch__caption text-base-body-13">
										No uncommitted changes<br />on this branch
									</h2>
								</div>
							</div>
						{/if}
					</div>

					<BranchCommits {isUnapplied} />
				</div>
			</ScrollableContainer>
			<div class="divider-line">
				<Resizer
					viewport={rsViewport}
					direction="right"
					minWidth={320}
					sticky
					defaultLineColor={$fileIdSelection.length == 1 ? 'transparent' : 'var(--clr-border-main)'}
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

	.branch-card__dropzone-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		position: relative;
	}

	.branch-card__contents {
		position: relative;
		display: flex;
		flex-direction: column;
		flex: 1;
		min-height: 100%;
		gap: var(--size-8);
		padding: var(--size-12);
	}

	.card {
		flex: 1;
	}

	.card-notifications {
		display: flex;
		flex-direction: column;
		padding: 0 var(--size-12) var(--size-12) var(--size-12);
	}

	.new-branch__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--size-8);
		max-width: 14rem;
	}

	.new-branch,
	.no-changes {
		user-select: none;
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		align-items: center;
		color: var(--clr-scale-ntrl-60);
		background: var(--clr-bg-main);
		justify-content: center;
		padding: var(--size-48) 0;
		border-radius: var(--radius-m);
		cursor: default; /* was defaulting to text cursor */
	}

	.no-changes {
		color: var(--clr-scale-ntrl-40);
		text-align: center;
	}

	.new-branch__title {
		color: var(--clr-scale-ntrl-40);
	}

	.new-branch__caption {
		color: var(--clr-scale-ntrl-50);
		opacity: 0.6;
	}

	.new-branch__caption,
	.new-branch__title {
		text-align: center;
	}

	.new-branch__image {
		width: 7.5rem;
		margin-bottom: var(--size-10);
	}

	/* hunks drop zone */
	/* cherry pick drop zone */
	/* move commit drop zone */
	/* squash drop zone */
	:global(
			.lane-dz-active .lane-dz-marker,
			.cherrypick-dz-active .cherrypick-dz-marker,
			.move-commit-dz-active .move-commit-dz-marker,
			.squash-dz-active .squash-dz-marker
		) {
		display: flex;
	}

	.branch-card :global(.contents) {
		display: flex;
		flex-direction: column;
		min-height: 100%;
	}

	/* COLLAPSED LANE */
	.collapsed-lane-wrapper {
		display: flex;
		flex-direction: column;
		padding: var(--size-12);
		height: 100%;
		border-right: 1px solid var(--clr-border-main);
	}
</style>
