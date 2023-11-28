<script lang="ts">
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import { getContext, onMount } from 'svelte';
	import { dropzone } from '$lib/utils/draggable';
	import {
		isDraggableHunk,
		isDraggableFile,
		isDraggableCommit,
		type DraggableCommit,
		type DraggableFile,
		type DraggableHunk
	} from '$lib/draggables';
	import { Ownership } from '$lib/vbranches/ownership';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { quintOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import type { User, getCloudApiClient } from '$lib/backend/cloud';
	import Resizer from '$lib/components/Resizer.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import lscache from 'lscache';
	import CommitDialog from './CommitDialog.svelte';
	import { writable, type Writable } from 'svelte/store';
	import { computedAddedRemoved } from '$lib/vbranches/fileStatus';
	import type { PrService } from '$lib/github/pullrequest';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import { isDraggableRemoteCommit, type DraggableRemoteCommit } from '$lib/draggables';
	import BranchHeader from './BranchHeader.svelte';
	import UpstreamCommits from './UpstreamCommits.svelte';
	import BranchFiles from './BranchFiles.svelte';
	import LocalCommits from './LocalCommits.svelte';

	const [send, receive] = crossfade({
		duration: (d) => Math.sqrt(d * 200),

		fallback(node) {
			const style = getComputedStyle(node);
			const transform = style.transform === 'none' ? '' : style.transform;

			return {
				duration: 600,
				easing: quintOut,
				css: (t) => `
					transform: ${transform} scale(${t});
					opacity: ${t}
				`
			};
		}
	});

	export let branch: Branch;
	export let readonly = false;
	export let projectId: string;
	export let base: BaseBranch | undefined | null;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let maximized = false;
	export let branchCount = 1;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let user: User | undefined;
	export let selectedFileId: Writable<string | undefined>;
	export let prService: PrService;

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	const allExpanded = writable(false);
	const allCollapsed = writable(false);
	let rsViewport: HTMLElement;
	let viewport: HTMLElement;
	let contents: HTMLElement;

	const laneWidthKey = 'laneWidth:';
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

	function handleCollapseAll() {
		branch.files.forEach((f) => setExpandedWithCache(f, false));
		$allExpanded = false;
		branch.files = branch.files;
	}

	function handleExpandAll() {
		branch.files.forEach((f) => setExpandedWithCache(f, true));
		$allExpanded = true;
		branch.files = branch.files;
	}

	let commitDialogShown = false;

	$: if (commitDialogShown && branch.files.length === 0) {
		commitDialogShown = false;
	}

	function generateBranchName() {
		const diff = branch.files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if (user) {
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

	function resetHeadCommit() {
		if (branch.commits.length > 1) {
			branchController.resetBranch(branch.id, branch.commits[1].id);
		} else if (branch.commits.length === 1 && base) {
			branchController.resetBranch(branch.id, base.baseSha);
		}
	}

	onMount(() => {
		expandFromCache();
		laneWidth = lscache.get(laneWidthKey + branch.id) ?? $userSettings.defaultLaneWidth;
	});

	const selectedOwnership = writable(Ownership.fromBranch(branch));
	$: if (commitDialogShown) selectedOwnership.set(Ownership.fromBranch(branch));

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
			const newOwnership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
			branchController.updateBranchOwnership(
				branch.id,
				(newOwnership + '\n' + branch.ownership).trim()
			);
		}
	}

	function acceptAmend(commit: Commit) {
		return (data: any) => {
			if (
				isDraggableHunk(data) &&
				data.branchId == branch.id &&
				commit.id == branch.commits.at(0)?.id
			) {
				return true;
			} else if (
				isDraggableFile(data) &&
				data.branchId == branch.id &&
				commit.id == branch.commits.at(0)?.id
			) {
				return true;
			} else {
				return false;
			}
		};
	}

	function onAmend(data: DraggableFile | DraggableHunk) {
		if (isDraggableHunk(data)) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			branchController.amendBranch(branch.id, newOwnership);
		} else if (isDraggableFile(data)) {
			const newOwnership = `${data.file.path}:${data.file.hunks.map(({ id }) => id).join(',')}`;
			branchController.amendBranch(branch.id, newOwnership);
		}
	}

	function acceptSquash(commit: Commit) {
		return (data: any) => {
			return (
				isDraggableCommit(data) &&
				data.branchId == branch.id &&
				(commit.parentIds.includes(data.commit.id) || data.commit.parentIds.includes(commit.id))
			);
		};
	}

	function onSquash(commit: Commit) {
		function isParentOf(commit: Commit, other: Commit) {
			return commit.parentIds.includes(other.id);
		}
		return (data: DraggableCommit) => {
			if (isParentOf(commit, data.commit)) {
				branchController.squashBranchCommit(data.branchId, commit.id);
			} else if (isParentOf(data.commit, commit)) {
				branchController.squashBranchCommit(data.branchId, data.commit.id);
			}
		};
	}
</script>

<div bind:this={rsViewport} class="resize-viewport">
	<div class="branch-card" style:width={maximized ? '100%' : `${laneWidth}px`}>
		<div class="flex flex-col">
			<BranchHeader
				{readonly}
				{branchController}
				{branch}
				{allCollapsed}
				{allExpanded}
				on:action={(e) => {
					if (e.detail == 'expand') {
						handleExpandAll();
					} else if (e.detail == 'collapse') {
						handleCollapseAll();
					} else if (e.detail == 'generate-branch-name') {
						generateBranchName();
					}
				}}
			/>

			{#if branch.upstream?.commits.length && branch.upstream?.commits.length > 0 && !branch.conflicted}
				<UpstreamCommits
					upstream={branch.upstream}
					branchId={branch.id}
					{branchController}
					{branchCount}
					{projectId}
					{base}
				/>
			{/if}
		</div>
		<div
			class="relative flex flex-grow overflow-y-hidden"
			use:dropzone={{
				hover: 'cherrypick-dz-hover',
				active: 'cherrypick-dz-active',
				accepts: acceptCherrypick,
				onDrop: onCherrypicked
			}}
			use:dropzone={{
				hover: 'lane-dz-hover',
				active: 'lane-dz-active',
				accepts: acceptBranchDrop,
				onDrop: onBranchDrop
			}}
		>
			<!-- TODO: Figure out why z-10 is necessary for expand up/down to not come out on top -->
			<div
				class="cherrypick-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
			>
				<div class="hover-text invisible font-semibold">Apply here</div>
			</div>

			<!-- TODO: Figure out why z-10 is necessary for expand up/down to not come out on top -->
			<div
				class="lane-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
			>
				<div class="hover-text invisible font-semibold">Move here</div>
			</div>
			<div bind:this={viewport} class="scroll-container hide-native-scrollbar">
				<div bind:this={contents} class="flex min-h-full flex-col">
					{#if branch.files?.length > 0}
						<BranchFiles {branch} {readonly} {selectedOwnership} {selectedFileId} />
						{#if branch.active}
							<CommitDialog
								{projectId}
								{branchController}
								{branch}
								{cloudEnabled}
								{cloud}
								{selectedOwnership}
								{user}
								on:action={(e) => {
									if (e.detail == 'generate-branch-name') {
										generateBranchName();
									}
								}}
							/>
						{/if}
					{:else if branch.commits.length == 0}
						<div class="new-branch" data-dnd-ignore>
							<h1 class="text-base-16 text-semibold">
								This is a new branch. Let's start creating!
							</h1>
							<p class="px-12">Get some work done, then throw some files my way!</p>
						</div>
					{:else}
						<!-- attention: these markers have custom css at the bottom of thise file -->
						<div class="no-changes" data-dnd-ignore>
							<h1 class="text-base-16 text-semibold">No uncommitted changes on this branch</h1>
						</div>
					{/if}
					{#if branch.commits.length > 0}
						<LocalCommits
							{branch}
							{base}
							{send}
							{receive}
							{githubContext}
							{projectId}
							{branchController}
							{acceptAmend}
							{acceptSquash}
							{onAmend}
							{onSquash}
							{resetHeadCommit}
							{prService}
							type="local"
						/>
						<LocalCommits
							{branch}
							{base}
							{send}
							{receive}
							{githubContext}
							{projectId}
							{branchController}
							{acceptAmend}
							{acceptSquash}
							{onAmend}
							{onSquash}
							{resetHeadCommit}
							{prService}
							type="remote"
						/>
						<LocalCommits
							{branch}
							{base}
							{send}
							{receive}
							{githubContext}
							{projectId}
							{branchController}
							{acceptAmend}
							{acceptSquash}
							{onAmend}
							{onSquash}
							{resetHeadCommit}
							{prService}
							type="integrated"
						/>
					{/if}
				</div>
			</div>
		</div>
	</div>
	{#if !maximized}
		<Resizer
			minWidth={330}
			viewport={rsViewport}
			direction="horizontal"
			class="z-30"
			on:width={(e) => {
				laneWidth = e.detail;
				lscache.set(laneWidthKey + branch.id, e.detail, 7 * 1440); // 7 day ttl
				userSettings.update((s) => ({
					...s,
					defaultLaneWidth: e.detail
				}));
				return true;
			}}
		/>
	{/if}
</div>

<style lang="postcss">
	.resize-viewport {
		position: relative;
		display: flex;
	}

	.branch-card {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		cursor: default;
		overflow-x: hidden;
		background: var(--clr-theme-container-light);
	}

	.scroll-container {
		max-height: 100%;
		flex-grow: 1;
		flex-direction: column;
		display: flex;
		overflow-y: scroll;
		overscroll-behavior: none;
	}

	.new-branch,
	.no-changes {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		color: var(--clr-theme-scale-ntrl-60);
		background: var(--clr-theme-container-light);
		justify-content: center;
		gap: var(--space-8);
		padding: 0 var(--space-40);
	}

	.new-branch h1 {
		color: var(--clr-theme-scale-ntrl-40);
		text-align: center;
	}

	.new-branch p {
		text-align: center;
		color: var(--clr-theme-scale-ntrl-50);
	}

	.no-changes h1 {
		text-align: center;
		text-align: center;
	}

	/* hunks drop zone */
	:global(.lane-dz-active .lane-dz-marker) {
		@apply flex;
	}
	:global(.lane-dz-hover .hover-text) {
		@apply visible;
	}

	/* cherry pick drop zone */
	:global(.cherrypick-dz-active .cherrypick-dz-marker) {
		@apply flex;
	}
	:global(.cherrypick-dz-hover .hover-text) {
		@apply visible;
	}

	/* squash drop zone */
	:global(.squash-dz-active .squash-dz-marker) {
		@apply flex;
	}
	:global(.squash-dz-hover .hover-text) {
		@apply visible;
	}
</style>
