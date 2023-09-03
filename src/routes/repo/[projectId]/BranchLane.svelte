<script lang="ts">
	import * as toasts from '$lib/toasts';
	import { userStore } from '$lib/stores/user';
	import type { BaseBranch, Branch, File } from '$lib/vbranches/types';
	import { getContext, onMount } from 'svelte';
	import { IconAISparkles } from '$lib/icons';
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
	import { invoke } from '@tauri-apps/api/tauri';
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

	const user = userStore;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	let commitMessage: string;

	$: remoteCommits = branch.commits.filter((c) => c.isRemote);
	$: localCommits = branch.commits.filter((c) => !c.isRemote);
	$: messageRows = Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10);

	let allExpanded: boolean | undefined;
	let isPushing = false;
	let meatballButton: HTMLDivElement;
	let textAreaInput: HTMLTextAreaElement;
	let viewport: Element;
	let contents: Element;
	let rsViewport: HTMLElement;
	let laneWidth: number;
	let deleteBranchModal: Modal;
	let applyConflictedModal: Modal;

	const dzType = 'text/hunk';
	const laneWidthKey = 'laneWidth:';

	function commit() {
		branchController.commitBranch(branch.id, commitMessage);
	}

	function push() {
		if (localCommits[0]?.id) {
			isPushing = true;
			branchController.pushBranch(branch.id).finally(() => (isPushing = false));
		}
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

	function url(target: BaseBranch | undefined, upstreamBranchName: string) {
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

	export function git_get_config(params: { key: string }) {
		return invoke<string>('git_get_global_config', params);
	}

	let annotateCommits = true;

	function checkCommitsAnnotated() {
		git_get_config({ key: 'gitbutler.utmostDiscretion' }).then((value) => {
			annotateCommits = value ? value === '0' : true;
		});
	}
	$: checkCommitsAnnotated();

	let isGeneratingCommigMessage = false;
	function trimNonLetters(input: string): string {
		const regex = /^[^a-zA-Z]+|[^a-zA-Z]+$/g;
		const trimmedString = input.replace(regex, '');
		return trimmedString;
	}
	async function generateCommitMessage(files: File[]) {
		const diff = files
			.map((f) => f.hunks)
			.flat()
			.map((h) => h.diff)
			.flat()
			.join('\n')
			.slice(0, 5000);

		if ($user === null) return;

		isGeneratingCommigMessage = true;
		cloud.summarize
			.commit($user.access_token, {
				diff,
				uid: projectId
			})
			.then(({ message }) => {
				const firstNewLine = message.indexOf('\n');
				const summary = firstNewLine > -1 ? message.slice(0, firstNewLine).trim() : message;
				const description = firstNewLine > -1 ? message.slice(firstNewLine + 1).trim() : '';
				commitMessage = trimNonLetters(
					description.length > 0 ? `${summary}\n\n${description}` : summary
				);
			})
			.catch(() => {
				toasts.error('Failed to generate commit message');
			})
			.finally(() => {
				isGeneratingCommigMessage = false;
			});
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
			}
		});
	});
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
		const ownership = branch.files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id).join(','))
			.join('\n');
		branchController.updateBranchOwnership(branch.id, (data + '\n' + ownership).trim());
	}}
>
	<div
		bind:this={rsViewport}
		class="flex flex-grow cursor-default flex-col overflow-x-hidden border-l border-r border-light-400 bg-light-150 dark:border-dark-600 dark:bg-dark-1000 dark:text-dark-100"
	>
		<div class="flex text-light-900 dark:bg-dark-800 dark:font-normal dark:text-dark-100">
			<div class="flex flex-grow flex-col border-b border-light-400 dark:border-dark-600">
				{#if !branch.mergeable}
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
							class="w-full truncate rounded border border-transparent bg-transparent px-1 font-mono font-bold text-light-800 hover:border-light-400 dark:text-dark-100 dark:hover:border-dark-600"
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
								class="scale-90 px-1 py-1 text-light-600 hover:text-light-800"
								title="Stash this branch"
								on:click={() => {
									if (branch.id) branchController.unapplyBranch(branch.id);
								}}
							>
								<IconCloseSmall />
							</button>
						{:else}
							{#if branch.mergeable}
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
					<div
						class="flex w-full flex-col border-t border-light-400 bg-light-200 dark:border-dark-400 dark:bg-dark-800"
						transition:slide={{ duration: 150 }}
					>
						{#if annotateCommits}
							<div class="bg-blue-400 p-2 text-sm text-white">
								GitButler will be the committer of this commit.
								<a
									target="_blank"
									rel="noreferrer"
									class="font-bold"
									href="https://docs.gitbutler.com/features/virtual-branches/committer-mark"
									>Learn more</a
								>
							</div>
						{/if}
						<div class="flex items-center">
							<textarea
								bind:this={textAreaInput}
								bind:value={commitMessage}
								on:dblclick|stopPropagation
								class="flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto border border-white bg-white p-2 font-mono text-dark-700 outline-none focus:border-purple-600 focus:ring-0 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400"
								placeholder="Your commit message here"
								rows={messageRows}
								required
							/>
						</div>
						<div class="flex flex-grow justify-end gap-2 p-3 px-5">
							<div>
								{#if cloudEnabled && $user}
									<Button
										disabled={isGeneratingCommigMessage}
										tabindex={-1}
										kind="outlined"
										class="text-light-500"
										height="small"
										id="generate-ai-message"
										icon={IconAISparkles}
										loading={isGeneratingCommigMessage}
										on:click={() => generateCommitMessage(branch.files)}
									>
										<span class="text-light-700">Generate message</span>
									</Button>
								{:else}
									<Tooltip
										label="Summary generation requres that you are logged in and have cloud sync enabled for the project"
									>
										<Button
											disabled={true}
											tabindex={-1}
											kind="outlined"
											class="text-light-500"
											height="small"
											icon={IconAISparkles}
											loading={isGeneratingCommigMessage}
										>
											<span class="text-light-700">Generate message</span>
										</Button>
									</Tooltip>
								{/if}
							</div>
							<Button
								class="w-20"
								height="small"
								color="purple"
								id="commit-to-branch"
								on:click={() => {
									if (commitMessage) commit();
									commitMessage = '';
									commitDialogShown = false;
								}}
							>
								Commit
							</Button>
						</div>
					</div>
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
						props: { files: branch.files }
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
				class="lane-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-light-50/75 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-dark-900/75 dark:outline-dark-300"
			>
				<div class="hover-text invisible font-semibold">Drop here to move hunk/file</div>
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
										{file}
										{dzType}
										{projectId}
										{projectPath}
										{branchController}
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
									class="no-changes space-y-6 rounded p-8 text-center text-light-700 dark:border-zinc-700"
									data-dnd-ignore
								>
									<p>Nothing on this branch yet.</p>
									{#if !readonly}
										<IconNewBadge class="mx-auto mt-4 h-16 w-16 text-blue-400 dark:text-dark-400" />
										<p class="px-12 text-light-600">
											Get some work done, then throw some files my way!
										</p>
									{/if}
								</div>
							{:else}
								<!-- attention: these markers have custom css at the bottom of thise file -->
								<div
									class="no-changes rounded text-center font-mono text-light-700 dark:border-zinc-700"
									data-dnd-ignore
								>
									No uncommitted changes on this branch
								</div>
							{/if}
						{/if}
					</div>
					{#if localCommits.length > 0 || remoteCommits.length > 0}
						<div
							class="flex w-full flex-grow flex-col gap-2 border-t border-light-400 dark:border-dark-500"
						>
							{#if localCommits.length > 0}
								<div
									class="relative"
									class:flex-grow={remoteCommits.length == 0}
									transition:slide={{ duration: 150 }}
								>
									<div
										class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-400 via-light-500 via-90% dark:from-dark-600 dark:via-dark-600"
										style={remoteCommits.length == 0 ? 'height: calc();' : 'height: 100%;'}
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
													<div
														class="h-3 w-3 rounded-full border-2 border-light-500 bg-light-200 dark:border-dark-600 dark:bg-dark-1000"
													/>
												</div>
												<CommitCard {commit} isIntegrated={commit.isRemote} />
											</div>
										{/each}
									</div>
								</div>
							{/if}
							{#if remoteCommits.length > 0}
								<div class="relative flex-grow">
									<div
										class="dark:form-dark-600 absolute top-4 ml-[0.75rem]
						w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
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
													href={url(base, branch.upstream)}
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
												<CommitCard
													{commit}
													url={base?.commitUrl(commit.id)}
													isIntegrated={commit.isIntegrated}
												/>
											</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{/if}
				</div>
			</div>
			<Scrollbar {viewport} {contents} width="0.4rem" />
		</div>
	</div>
	{#if !maximized}
		<Resizer
			minWidth={180}
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
	:global(.lane-dz-hover .lane-dz-marker) {
		/* TODO: Why doesn't hover:outline-light-800 work on the element? */
		@apply text-light-800 outline-light-700;
	}
	:global(.dark .lane-dz-hover .lane-dz-marker) {
		@apply text-dark-100 outline-dark-200;
	}
</style>
