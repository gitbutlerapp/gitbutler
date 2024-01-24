<script lang="ts">
	import type { BaseBranch, Branch, File } from '$lib/vbranches/types';
	import { getContext, onMount } from 'svelte';
	import { dropzone } from '$lib/utils/draggable';
	import {
		isDraggableHunk,
		isDraggableFile,
		type DraggableFile,
		type DraggableHunk
	} from '$lib/draggables';
	import { filesToOwnership, type Ownership } from '$lib/vbranches/ownership';
	import { getExpandedWithCacheFallback } from './cache';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import Resizer from '$lib/components/Resizer.svelte';
	import lscache from 'lscache';
	import CommitDialog from './CommitDialog.svelte';
	import { get, writable, type Writable } from 'svelte/store';
	import { computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import type { GitHubService } from '$lib/github/service';
	import { isDraggableRemoteCommit, type DraggableRemoteCommit } from '$lib/draggables';
	import BranchHeader from './BranchHeader.svelte';
	import BranchFiles from './BranchFiles.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { persisted } from '$lib/persisted/persisted';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import BranchCommits from './BranchCommits.svelte';
	import type { Project } from '$lib/backend/projects';
	import ImgThemed from '$lib/components/ImgThemed.svelte';

	import DropzoneOverlay from './DropzoneOverlay.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import type { BranchService } from '$lib/branches/service';

	export let branch: Branch;
	export let readonly = false;
	export let project: Project;
	export let base: BaseBranch | undefined | null;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let branchService: BranchService;
	export let branchCount = 1;
	export let user: User | undefined;
	export let selectedFiles: Writable<File[]>;
	export let githubService: GitHubService;
	export let selectedOwnership: Writable<Ownership>;
	export let commitBoxOpen: Writable<boolean>;

	const allExpanded = writable(false);
	const allCollapsed = writable(false);
	const aiGenEnabled = projectAiGenEnabled(project.id);

	let rsViewport: HTMLElement;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const defaultBranchWidthRem = persisted<number | undefined>(24, 'defaulBranchWidth' + project.id);
	const laneWidthKey = 'laneWidth_';

	let laneWidth: number;

	$: {
		// On refresh we need to check expansion status from localStorage
		branch.files && expandFromCache();
	}

	function expandFromCache() {
		// Exercise cache lookup for all files.
		$allExpanded = branch.files.every((f) => getExpandedWithCacheFallback(f));
		$allCollapsed = branch.files.every((f) => getExpandedWithCacheFallback(f) == false);
	}

	$: if ($commitBoxOpen && branch.files.length === 0) {
		$commitBoxOpen = false;
	}

	function generateBranchName() {
		const diff = branch.files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if (user && aiGenEnabled) {
			cloud.summarize.branch(user.access_token, { diff }).then((result) => {
				if (result.message && result.message !== branch.name) {
					branch.name = result.message;
					branchController.updateBranchName(branch.id, branch.name);
				}
			});
		}
	}

	$: linesTouched = computedAddedRemoved(...branch.files);
	$: if (
		branch.name.toLowerCase().includes('virtual branch') &&
		linesTouched.added + linesTouched.removed > 4
	) {
		generateBranchName();
	}

	onMount(() => {
		expandFromCache();
		laneWidth = lscache.get(laneWidthKey + branch.id);
	});

	function acceptCherrypick(data: any) {
		return isDraggableRemoteCommit(data) && data.branchId == branch.id;
	}

	function onCherrypicked(data: DraggableRemoteCommit) {
		branchController.cherryPick(branch.id, data.remoteCommit.id);
	}

	function acceptBranchDrop(data: any) {
		if (isDraggableHunk(data) && data.branchId != branch.id) {
			return true;
		} else if (isDraggableFile(data) && data.branchId != branch.id) {
			return true;
		} else {
			return false;
		}
	}

	function onBranchDrop(data: DraggableHunk | DraggableFile) {
		if (isDraggableHunk(data)) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.updateBranchOwnership(
				branch.id,
				(newOwnership + '\n' + branch.ownership).trim()
			);
		} else if (isDraggableFile(data)) {
			let files = get(data.files);
			if (files.length == 0) {
				files = [data.current];
			}
			const newOwnership = filesToOwnership(files);
			branchController.updateBranchOwnership(
				branch.id,
				(newOwnership + '\n' + branch.ownership).trim()
			);
		}
	}
</script>

<div bind:this={rsViewport} class="branch-card resize-viewport" data-tauri-drag-region>
	<ScrollableContainer>
		<div style:width={`${laneWidth || $defaultBranchWidthRem}rem`} class="branch-card__contents">
			<BranchHeader
				{readonly}
				{branchController}
				{branch}
				{base}
				{githubService}
				projectId={project.id}
				on:action={(e) => {
					if (e.detail == 'generate-branch-name') {
						generateBranchName();
					}
				}}
			/>
			<!-- DROPZONES -->
			<DropzoneOverlay class="cherrypick-dz-marker" label="Apply here" />
			<DropzoneOverlay class="lane-dz-marker" label="Move here" />

			<div
				class="branch-card__dropzone-wrapper"
				use:dropzone={{
					hover: 'cherrypick-dz-hover',
					active: 'cherrypick-dz-active',
					accepts: acceptCherrypick,
					onDrop: onCherrypicked,
					disabled: readonly
				}}
				use:dropzone={{
					hover: 'lane-dz-hover',
					active: 'lane-dz-active',
					accepts: acceptBranchDrop,
					onDrop: onBranchDrop,
					disabled: readonly
				}}
			>
				<DropzoneOverlay class="cherrypick-dz-marker" label="Apply here" />
				<DropzoneOverlay class="lane-dz-marker" label="Move here" />
				{#if branch.files?.length > 0}
					<div class="card">
						<BranchFiles
							{branch}
							{readonly}
							{selectedOwnership}
							{selectedFiles}
							showCheckboxes={$commitBoxOpen}
						/>
						{#if branch.active}
							<CommitDialog
								projectId={project.id}
								{branchController}
								{branch}
								{cloud}
								{selectedOwnership}
								{user}
								bind:expanded={commitBoxOpen}
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
								<ImgThemed
									imgSet={{
										light: '/images/lane-new-light.webp',
										dark: '/images/lane-new-dark.webp'
									}}
								/>
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
								<ImgThemed
									imgSet={{
										light: '/images/lane-no-changes-light.webp',
										dark: '/images/lane-no-changes-dark.webp'
									}}
								/>
							</div>
							<h2 class="new-branch__caption text-base-body-13">
								No uncommitted changes<br />on this branch
							</h2>
						</div>
					</div>
				{/if}
			</div>
			<BranchCommits
				{base}
				{branch}
				{project}
				{githubService}
				{branchController}
				{branchService}
				{branchCount}
				{readonly}
			/>
		</div>
	</ScrollableContainer>

	<Resizer
		viewport={rsViewport}
		direction="right"
		inside={$selectedFiles.length > 0}
		minWidth={320}
		on:width={(e) => {
			laneWidth = e.detail / (16 * $userSettings.zoom);
			lscache.set(laneWidthKey + branch.id, laneWidth, 7 * 1440); // 7 day ttl
			$defaultBranchWidthRem = laneWidth;
		}}
	/>
</div>

<style lang="postcss">
	.resize-viewport {
		height: 100%;
		position: relative;
		display: flex;
	}

	.branch-card {
		display: flex;
		flex-direction: column;
		user-select: none;
	}

	.branch-card__dropzone-wrapper {
		position: relative;
	}

	.branch-card__contents {
		display: flex;
		flex-direction: column;
		padding-top: 20px;
		gap: var(--space-4);
		padding: var(--space-16) var(--space-8) var(--space-16) var(--space-8);
	}

	.resize-viewport {
		position: relative;
	}

	.new-branch__content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-8);
		max-width: 14rem;
	}

	.new-branch,
	.no-changes {
		user-select: none;
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-60);
		background: var(--clr-theme-container-light);
		justify-content: center;
		padding: var(--space-48) 0;
		border-radius: var(--radius-m);
	}

	.no-changes {
		color: var(--clr-theme-scale-ntrl-40);
		text-align: center;
	}

	.new-branch__title {
		color: var(--clr-theme-scale-ntrl-40);
	}

	.new-branch__caption {
		color: var(--clr-theme-scale-ntrl-50);
		opacity: 0.6;
	}

	.new-branch__caption,
	.new-branch__title {
		text-align: center;
	}

	.new-branch__image {
		width: 7.5rem;
		margin-bottom: var(--space-10);
	}

	/* hunks drop zone */
	:global(.lane-dz-active .lane-dz-marker) {
		display: flex;
	}

	/* cherry pick drop zone */
	:global(.cherrypick-dz-active .cherrypick-dz-marker) {
		@apply flex;
	}

	/* squash drop zone */
	:global(.squash-dz-active .squash-dz-marker) {
		@apply flex;
	}
</style>
