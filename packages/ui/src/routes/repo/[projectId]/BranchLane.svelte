<script lang="ts">
	import { userStore } from '$lib/stores/user';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import { getContext, onMount } from 'svelte';
	import { Ownership } from '$lib/vbranches/ownership';
	import { Button, Link, Modal, Tooltip } from '$lib/components';
	import IconKebabMenu from '$lib/icons/IconKebabMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches/branchController';
	import FileCard from './FileCard.svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { crossfade, fade } from 'svelte/transition';
	import { flip } from 'svelte/animate';
	import type { getCloudApiClient } from '$lib/api/cloud/api';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import IconNewBadge from '$lib/icons/IconNewBadge.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import lscache from 'lscache';
	import IconCloseSmall from '$lib/icons/IconCloseSmall.svelte';
	import Tabs from './Tabs.svelte';
	import NotesTabPanel from './NotesTabPanel.svelte';
	import FileTreeTabPanel from './FileTreeTabPanel.svelte';
	import BranchLanePopupMenu from './BranchLanePopupMenu.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import IconBackspace from '$lib/icons/IconBackspace.svelte';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import CommitDialog from './CommitDialog.svelte';
	import { writable } from 'svelte/store';
	import { computedAddedRemoved } from '$lib/vbranches/fileStatus';

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
	export let projectPath: string;
	export let projectId: string;
	export let base: BaseBranch | undefined;
	export let cloudEnabled: boolean;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let branchController: BranchController;
	export let maximized = false;
	export let branchCount = 1;

	const user = userStore;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	$: localCommits = branch.commits.filter((c) => !c.isRemote);
	$: remoteCommits = branch.commits.filter((c) => c.isRemote && !c.isIntegrated);
	$: integratedCommits = branch.commits.filter((c) => c.isIntegrated);

	let allExpanded: boolean | undefined;
	let isPushing = false;
	let meatballButton: HTMLDivElement;
	let viewport: Element;
	let contents: Element;
	let rsViewport: HTMLElement;
	let laneWidth: number;
	let deleteBranchModal: Modal;
	let applyConflictedModal: Modal;

	const dzType = 'text/hunk';
	const laneWidthKey = 'laneWidth:';

	function push() {
		if (localCommits[0]?.id) {
			isPushing = true;
			branchController.pushBranch(branch.id).finally(() => (isPushing = false));
		}
	}

	function merge() {
		console.log(`merge ${branch.id}`);
		branchController.mergeUpstream(branch.id);
	}

	$: {
		// On refresh we need to check expansion status from localStorage
		branch.files && expandFromCache();
	}

	function expandFromCache() {
		// Exercise cache lookup for all files.
		branch.files.forEach((f) => getExpandedWithCacheFallback(f));
		if (branch.files.every((f) => getExpandedWithCacheFallback(f))) {
			allExpanded = true;
		} else if (branch.files.every((f) => getExpandedWithCacheFallback(f) === false)) {
			allExpanded = false;
		} else {
			allExpanded = undefined;
		}
	}

	$: allCollapsed = branch.files.every((f) => getExpandedWithCacheFallback(f) === false);

	function handleCollapseAll() {
		branch.files.forEach((f) => setExpandedWithCache(f, false));
		allExpanded = false;
		branch.files = branch.files;
	}

	function handleExpandAll() {
		branch.files.forEach((f) => setExpandedWithCache(f, true));
		allExpanded = true;
		branch.files = branch.files;
	}

	function handleBranchNameChange() {
		branchController.updateBranchName(branch.id, branch.name);
	}

	function baseUrl(target: BaseBranch | undefined) {
		if (!target) return undefined;
		const parts = target.branchName.split('/');
		return `${target.repoBaseUrl}/commits/${parts[parts.length - 1]}`;
	}

	function branchUrl(target: BaseBranch | undefined, upstreamBranchName: string) {
		if (!target) return undefined;
		const baseBranchName = target.branchName.split('/')[1];
		const parts = upstreamBranchName.split('/');
		const branchName = parts[parts.length - 1];
		return `${target.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}

	let commitDialogShown = false;

	$: if (commitDialogShown && branch.files.length === 0) {
		commitDialogShown = false;
	}

	let upstreamCommitsShown = false;

	$: if (upstreamCommitsShown && branch.upstreamCommits.length === 0) {
		upstreamCommitsShown = false;
	}

	function generateBranchName() {
		const diff = branch.files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if ($user) {
			cloud.summarize.branch($user.access_token, { diff }).then((result) => {
				console.log(result);
				if (result.message && result.message !== branch.name) {
					branch.name = result.message;
					handleBranchNameChange();
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

	// We have to create this manually for now.
	// TODO: Use document.body.addEventListener to avoid having to use backdrop
	let popupMenu = new BranchLanePopupMenu({
		target: document.body,
		props: { allExpanded, allCollapsed, order: branch?.order, branchController }
	});

	function toggleBranch(branch: Branch) {
		if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
	}

	onMount(() => {
		expandFromCache();
		laneWidth = lscache.get(laneWidthKey + branch.id) ?? $userSettings.defaultLaneWidth;
		return popupMenu.$on('action', (e) => {
			if (e.detail == 'expand') {
				handleExpandAll();
			} else if (e.detail == 'collapse') {
				handleCollapseAll();
			} else if (e.detail == 'generate-branch-name') {
				generateBranchName();
			}
		});
	});

	const selectedOwnership = writable(Ownership.fromBranch(branch));
	$: if (commitDialogShown) selectedOwnership.set(Ownership.fromBranch(branch));
</script>

<div
	class="flex h-full shrink-0 snap-center"
	style:width={maximized ? '100%' : `${laneWidth}px`}
	draggable={!readonly}
	role="group"
	use:dzHighlight={{ type: dzType, hover: 'lane-dz-hover', active: 'lane-dz-active' }}
	on:dragstart
	on:dragend
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const data = e.dataTransfer.getData(dzType);
		const [newFileId, newHunks] = data.split(':');
		const existingHunkIds =
			branch.files.find((f) => f.id === newFileId)?.hunks.map((h) => h.id) || [];
		const newHunkIds = newHunks.split(',').filter((h) => !existingHunkIds.includes(h));
		if (newHunkIds.length == 0) {
			// don't allow dropping hunk to the line where it already is
			return;
		}
		branchController.updateBranchOwnership(branch.id, (data + '\n' + branch.ownership).trim());
	}}
>
	<div
		bind:this={rsViewport}
		class="bg-color-3 border-color-4 flex flex-grow cursor-default flex-col overflow-x-hidden border-l border-r"
	>
		<div class="flex">
			<div class="bg-color-4 border-color-4 flex flex-grow flex-col border-b">
				{#await branch.isMergeable then isMergeable}
					{#if !isMergeable}
						<!-- use of relative is for tooltip rendering -->
						<div class="bg-red-500 px-2 py-0.5 text-center font-bold dark:bg-red-700">
							<Tooltip label="Canflicts with changes in your working directory, cannot be applied">
								<span class="text-white">cannot be applied</span>
							</Tooltip>
						</div>
					{:else if !branch.baseCurrent}
						<div class="bg-yellow-500 px-2 py-0.5 font-bold dark:bg-yellow-600">
							<Tooltip label="Will introduce merge conflicts if applied">
								<span class="">will cause merge conflicts</span>
							</Tooltip>
						</div>
					{/if}
				{/await}
				<div class="flex w-full items-center py-1 pl-1.5">
					{#if !readonly}
						<div bind:this={meatballButton}>
							<IconButton
								icon={IconKebabMenu}
								title=""
								class="flex h-6 w-3 flex-grow-0 scale-90 items-center justify-center"
								on:click={() => popupMenu.openByElement(meatballButton, branch.id)}
							/>
						</div>
					{/if}
					<div class="flex-grow pr-2">
						<input
							type="text"
							bind:value={branch.name}
							on:change={handleBranchNameChange}
							title={branch.name}
							class="text-color-3 hover:text-color-2 focus:text-color-2 hover:border-color-4 w-full truncate rounded border border-transparent bg-transparent px-1 font-mono font-bold"
							on:dblclick|stopPropagation
							on:click={(e) => e.currentTarget.select()}
						/>
					</div>
					<div class="flex gap-x-1 px-1" transition:fade={{ duration: 150 }}>
						{#if !readonly}
							{#if branch.files.length > 0}
								<Button
									class="w-20"
									height="small"
									kind="outlined"
									color="purple"
									disabled={branch.files.length == 0}
									on:click={() => (commitDialogShown = !commitDialogShown)}
								>
									<span class="purple">
										{#if !commitDialogShown}
											Commit
										{:else}
											Cancel
										{/if}
									</span>
								</Button>
							{/if}
							<button
								class="text-color-4 hover:text-color-2 scale-90 px-1 py-1"
								title="Stash this branch"
								on:click={() => {
									if (branch.id) branchController.unapplyBranch(branch.id);
								}}
							>
								<IconCloseSmall />
							</button>
						{:else}
							{#await branch.isMergeable then isMergeable}
								{#if isMergeable}
									<Button
										class="w-20"
										height="small"
										kind="outlined"
										color="purple"
										on:click={() => toggleBranch(branch)}
									>
										<span class="purple"> Apply </span>
									</Button>
								{/if}
							{/await}
							<IconButton
								icon={IconBackspace}
								class="px-1 py-1 align-middle "
								title="delete branch"
								on:click={() => deleteBranchModal.show(branch)}
							/>
						{/if}
					</div>
				</div>

				{#if commitDialogShown}
					<CommitDialog
						on:close={() => (commitDialogShown = false)}
						{projectId}
						{branchController}
						{branch}
						{cloudEnabled}
						{cloud}
						ownership={$selectedOwnership}
						user={$user}
					/>
				{/if}

				{#if branch.upstreamCommits.length > 0 && !branch.conflicted}
					<div class="bg-zinc-300 p-2 dark:bg-zinc-800">
						<div class="flex flex-row justify-between">
							<div class="p-1 text-purple-700">
								{branch.upstreamCommits.length}
								upstream {branch.upstreamCommits.length > 1 ? 'commits' : 'commit'}
							</div>
							<Button
								class="w-20"
								height="small"
								kind="outlined"
								color="purple"
								on:click={() => (upstreamCommitsShown = !upstreamCommitsShown)}
							>
								<span class="purple">
									{#if !upstreamCommitsShown}
										View
									{:else}
										Cancel
									{/if}
								</span>
							</Button>
						</div>
					</div>
					{#if upstreamCommitsShown}
						<div
							class="flex w-full flex-col border-t border-light-400 bg-light-300 p-2 dark:border-dark-400 dark:bg-dark-800"
							id="upstreamCommits"
						>
							<div class="bg-light-100">
								{#each branch.upstreamCommits as commit}
									<CommitCard {commit} {projectId} />
								{/each}
							</div>
							<div class="flex justify-end p-2">
								{#if branchCount > 1}
									<div class="px-2 text-sm">
										You have {branchCount} active branches. To merge upstream work, we will unapply all
										other branches.
									</div>
								{/if}
								<Button class="w-20" height="small" color="purple" on:click={merge}>Merge</Button>
							</div>
						</div>
					{/if}
				{/if}
			</div>
		</div>
		{#if branch.files.length !== 0}
			<Tabs
				branchId={branch.id}
				items={[
					{
						name: 'files',
						displayName: 'Changed files (' + branch.files.length + ')',
						component: FileTreeTabPanel,
						props: {
							files: branch.files,
							selectedOwnership,
							withCheckboxes: commitDialogShown
						}
					},
					{
						name: 'notes',
						displayName: 'Notes',
						component: NotesTabPanel,
						props: { notes: branch.notes, branchId: branch.id, branchController }
					}
				]}
			/>
		{/if}
		<div class="relative flex flex-grow overflow-y-hidden">
			<!-- TODO: Figure out why z-10 is necessary for expand up/down to not come out on top -->
			<div
				class="lane-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
			>
				<div class="hover-text invisible font-semibold">Move here</div>
			</div>
			<div
				bind:this={viewport}
				class="hide-native-scrollbar flex max-h-full flex-grow flex-col overflow-y-scroll pb-8"
			>
				<div bind:this={contents}>
					{#if branch.conflicted}
						<div class="mb-2 bg-red-500 p-2 font-bold text-white">
							{#if branch.files.some((f) => f.conflicted)}
								This virtual branch conflicts with upstream changes. Please resolve all conflicts
								and commit before you can continue.
							{:else}
								Please commit your resolved conflicts to continue.
							{/if}
						</div>
					{/if}

					<div class="flex flex-col py-2">
						{#if branch.files.length > 0}
							<div
								class="flex flex-shrink flex-col gap-y-2"
								transition:slide={{ duration: readonly ? 0 : 250 }}
							>
								<!-- TODO: This is an experiment in file sorting. Accept or reject! -->
								{#each sortLikeFileTree(branch.files) as file (file.id)}
									<FileCard
										expanded={file.expanded}
										conflicted={file.conflicted}
										{selectedOwnership}
										{file}
										{dzType}
										{projectId}
										{projectPath}
										{branchController}
										selectable={commitDialogShown}
										{readonly}
										on:expanded={(e) => {
											setExpandedWithCache(file, e.detail);
											expandFromCache();
										}}
									/>
								{/each}
							</div>
						{/if}
						{#if branch.files.length == 0}
							{#if branch.commits.length == 0}
								<div
									class="no-changes text-color-3 space-y-6 rounded p-8 text-center"
									data-dnd-ignore
								>
									<p>Nothing on this branch yet.</p>
									{#if !readonly}
										<IconNewBadge class="mx-auto mt-4 h-16 w-16 text-blue-400" />
										<p class="px-12">Get some work done, then throw some files my way!</p>
									{/if}
								</div>
							{:else}
								<!-- attention: these markers have custom css at the bottom of thise file -->
								<div class="no-changes text-color-3 rounded text-center font-mono" data-dnd-ignore>
									No uncommitted changes on this branch
								</div>
							{/if}
						{/if}
					</div>
					{#if localCommits.length > 0 || remoteCommits.length > 0}
						<div class="flex w-full flex-grow flex-col gap-2">
							{#if localCommits.length > 0}
								<div
									class="relative"
									class:flex-grow={remoteCommits.length == 0}
									transition:slide={{ duration: 150 }}
								>
									<div
										class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-400 via-light-500 via-90% dark:from-dark-600 dark:via-dark-600"
										style={localCommits.length == 0 ? 'height: calc();' : 'height: 100%;'}
									/>

									<div class="relative flex flex-col gap-2">
										<div
											class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-400 via-light-400 via-10% dark:from-dark-600 dark:via-dark-600"
										/>
										<div class="ml-10 mr-2 flex items-center py-2">
											<div
												class="ml-2 flex-grow font-mono text-sm font-bold text-dark-300 dark:text-light-300"
											>
												local
											</div>
											<Button
												class="w-20"
												height="small"
												kind="outlined"
												color="purple"
												id="push-commits"
												loading={isPushing}
												on:click={push}
											>
												<span class="purple">Push</span>
											</Button>
										</div>

										{#each localCommits as commit (commit.id)}
											<div
												class="flex w-full items-center pb-2 pr-1.5"
												in:receive={{ key: commit.id }}
												out:send={{ key: commit.id }}
												animate:flip
											>
												<div class="ml-[0.4rem] mr-1.5">
													<div class="border-color-4 h-3 w-3 rounded-full border-2" />
												</div>
												<CommitCard {projectId} {commit} />
											</div>
										{/each}
									</div>
								</div>
							{/if}
							{#if remoteCommits.length > 0}
								<div class="relative flex-grow">
									<div
										class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
										style="height: calc(100% - 1rem);"
									/>

									<div class="relative flex flex-grow flex-col gap-2">
										<div
											class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-600 via-light-600 via-10% dark:from-dark-400 dark:via-dark-400"
										/>

										<div
											class="relative max-w-full flex-grow overflow-hidden py-2 pl-12 pr-2 font-mono text-sm"
										>
											{#if branch.upstream}
												<Link
													target="_blank"
													rel="noreferrer"
													href={branchUrl(base, branch.upstream)}
													class="inline-block max-w-full truncate text-sm font-bold"
												>
													{branch.upstream.split('refs/remotes/')[1]}
												</Link>
											{/if}
										</div>

										{#each remoteCommits as commit (commit.id)}
											<div
												class="flex w-full items-center pb-2 pr-1.5"
												in:receive={{ key: commit.id }}
												out:send={{ key: commit.id }}
												animate:flip
											>
												<div class="ml-[0.4rem] mr-1.5">
													<div
														class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
														class:bg-light-500={commit.isRemote}
														class:dark:bg-dark-500={commit.isRemote}
													/>
												</div>
												<CommitCard {projectId} {commit} />
											</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
						{#if integratedCommits.length > 0}
							<div class="relative flex-grow">
								<div
									class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
									style="height: calc(100% - 1rem);"
								/>

								<div class="relative flex flex-grow flex-col gap-2">
									<div
										class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-600 via-light-600 via-10% dark:from-dark-400 dark:via-dark-400"
									/>

									<div
										class="relative max-w-full flex-grow overflow-hidden py-2 pl-12 pr-2 font-mono text-sm"
									>
										<Link
											target="_blank"
											rel="noreferrer"
											href={baseUrl(base)}
											class="inline-block max-w-full truncate text-sm font-bold"
										>
											{base?.branchName}
										</Link>
									</div>

									{#each integratedCommits as commit (commit.id)}
										<div
											class="flex w-full items-center pb-2 pr-1.5"
											in:receive={{ key: commit.id }}
											out:send={{ key: commit.id }}
											animate:flip
										>
											<div class="ml-[0.4rem] mr-1.5">
												<div
													class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
													class:bg-light-500={commit.isRemote}
													class:dark:bg-dark-500={commit.isRemote}
												/>
											</div>
											<CommitCard {projectId} {commit} />
										</div>
									{/each}
								</div>
							</div>
						{/if}
					{/if}
				</div>
			</div>
			<Scrollbar {viewport} {contents} width="0.4rem" />
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
			}}
		/>
	{/if}
</div>

<!-- Delete branch confirmation modal -->

<Modal width="small" bind:this={deleteBranchModal} let:item>
	<svelte:fragment slot="title">Delete branch</svelte:fragment>
	<div>
		Deleting <code>{item.name}</code> cannot be undone.
	</div>
	<svelte:fragment slot="controls" let:close let:item>
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="destructive"
			on:click={() => {
				branchController.deleteBranch(item.id);
				close();
			}}
		>
			Delete
		</Button>
	</svelte:fragment>
</Modal>

<Modal width="small" bind:this={applyConflictedModal}>
	<svelte:fragment slot="title">Merge conflicts</svelte:fragment>
	<p>Applying this branch will introduce merge conflicts.</p>
	<svelte:fragment slot="controls" let:item let:close>
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="purple"
			on:click={() => {
				branchController.applyBranch(item.id);
				close();
			}}
		>
			Update
		</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	:global(.lane-dz-active .lane-dz-marker) {
		@apply flex;
	}
	:global(.lane-dz-hover .hover-text) {
		@apply visible;
	}
</style>
