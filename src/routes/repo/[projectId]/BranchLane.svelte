<script lang="ts">
	import type { Commit, File } from '$lib/vbranches';
	import { onMount } from 'svelte';
	import FileCard from './FileCard.svelte';
	import { IconBranch } from '$lib/icons';
	import { Button } from '$lib/components';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import PopupMenu from '../../../lib/components/PopupMenu/PopupMenu.svelte';
	import PopupMenuItem from '../../../lib/components/PopupMenu/PopupMenuItem.svelte';
	import { dzHighlight } from './dropZone';
	import type { BranchController } from '$lib/vbranches';

	export let branchId: string;
	export let projectPath: string;
	export let name: string;
	export let commitMessage: string;
	export let upstream: string;
	export let files: File[];
	export let commits: Commit[];
	export let projectId: string;
	export let order: number;
	export let branchController: BranchController;

	$: remoteCommits = commits.filter((c) => c.isRemote);
	$: localCommits = commits.filter((c) => !c.isRemote);
	$: messageRows = Math.min(Math.max(commitMessage ? commitMessage.split('\n').length : 0, 1), 10);

	let allExpanded: boolean | undefined;
	let maximized = false;
	let isPushing = false;
	let popupMenu: PopupMenu;
	let meatballButton: HTMLButtonElement;

	const hoverClass = 'drop-zone-hover';
	const dzType = 'text/hunk';

	function commit() {
		console.log('commit', commitMessage, projectId, branchId);
		branchController.commitBranch(branchId, commitMessage);
	}

	function push() {
		if (localCommits[0]?.id) {
			console.log(`pushing ${branchId}`);
			isPushing = true;
			branchController.pushBranch(branchId).finally(() => (isPushing = false));
		}
	}

	onMount(() => {
		expandFromCache();
	});

	$: {
		// On refresh we need to check expansion status from localStorage
		files && expandFromCache();
	}

	function expandFromCache() {
		// Exercise cache lookup for all files.
		files.forEach((f) => getExpandedWithCacheFallback(f));
		if (files.every((f) => getExpandedWithCacheFallback(f))) {
			allExpanded = true;
		} else if (files.every((f) => getExpandedWithCacheFallback(f) === false)) {
			allExpanded = false;
		} else {
			allExpanded = undefined;
		}
	}
	function handleToggleExpandAll() {
		if (allExpanded == true) {
			files.forEach((f) => setExpandedWithCache(f, false));
			allExpanded = false;
		} else {
			files.forEach((f) => setExpandedWithCache(f, true));
			allExpanded = true;
		}
		files = files;
	}

	function handleBranchNameChange() {
		console.log('branch name change:', name);
		branchController.updateBranchName(branchId, name);
	}
</script>

<div
	draggable="true"
	class:w-full={maximized}
	class:w-96={!maximized}
	class="flex max-h-full min-w-[24rem] max-w-[120ch] shrink-0 cursor-grabbing snap-center flex-col overflow-y-auto bg-light-200 py-2 px-3 transition-width dark:bg-dark-1000 dark:text-dark-100"
	use:dzHighlight={{ type: dzType, hover: hoverClass, active: 'drop-zone-active' }}
	on:dragstart
	on:dragend
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const data = e.dataTransfer.getData(dzType);
		const [newFileId, newHunks] = data.split(':');
		const existingHunkIds = files.find((f) => f.id === newFileId)?.hunks.map((h) => h.id) || [];
		const newHunkIds = newHunks.split(',').filter((h) => !existingHunkIds.includes(h));
		if (newHunkIds.length == 0) {
			// don't allow dropping hunk to the line where it already is
			return;
		}
		const ownership = files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id).join(','))
			.join('\n');
		branchController.updateBranchOwnership(branchId, (data + '\n' + ownership).trim());
	}}
>
	<div
		class="mb-2 flex w-full shrink-0 items-center rounded bg-light-200 text-lg text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
	>
		<div
			on:dblclick={() => (maximized = !maximized)}
			class="h-8 w-8 flex-grow-0 cursor-pointer p-2 text-light-600 dark:text-dark-200"
		>
			<IconBranch />
		</div>
		<div class="mr-1 flex-grow ">
			<input
				type="text"
				bind:value={name}
				on:change={handleBranchNameChange}
				title={name}
				class="w-full truncate border-0 bg-light-200 font-bold text-light-900 dark:bg-dark-1000 dark:text-dark-100"
			/>
		</div>
		<button
			bind:this={meatballButton}
			class="h-8 w-8 flex-grow-0 p-2 text-light-600 transition-colors hover:bg-zinc-300 dark:text-dark-200 dark:hover:bg-zinc-800"
			on:keydown={() => popupMenu.openByElement(meatballButton, branchId)}
			on:click={() => popupMenu.openByElement(meatballButton, branchId)}
		>
			<IconMeatballMenu />
		</button>
	</div>

	<PopupMenu bind:this={popupMenu} let:item={branchId}>
		<PopupMenuItem on:click={() => branchId && branchController.unapplyBranch(branchId)}>
			Unapply
		</PopupMenuItem>

		<PopupMenuItem on:click={handleToggleExpandAll}>
			{#if allExpanded}
				Collapse all
			{:else}
				Expand all
			{/if}
		</PopupMenuItem>

		<div class="mx-3">
			<div class="my-2 h-[0.0625rem] w-full  bg-light-300 dark:bg-dark-500" />
		</div>

		<PopupMenuItem on:click={() => branchController.createBranch({ order })}>
			Create branch before
		</PopupMenuItem>

		<PopupMenuItem on:click={() => branchController.createBranch({ order: order + 1 })}>
			Create branch after
		</PopupMenuItem>
	</PopupMenu>

	<div
		class="flex flex-col rounded bg-white p-2 shadow-lg dark:border dark:border-dark-600 dark:bg-dark-800"
	>
		<div class="mb-2 flex items-center">
			<textarea
				bind:value={commitMessage}
				class="shrink-0 flex-grow cursor-text resize-none overflow-x-auto overflow-y-auto rounded border border-white bg-white p-2 text-dark-700 outline-none hover:border-light-400 focus:border-light-400 focus:ring-0 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400 dark:hover:border-dark-300 dark:focus:border-dark-300"
				placeholder="Your commit message here..."
				rows={messageRows}
			/>
		</div>
		<div class="mb-2 text-right">
			<Button
				height="small"
				color="purple"
				on:click={() => {
					commit();
				}}>Commit</Button
			>
		</div>
		<div class="flex flex-shrink flex-col gap-y-2">
			<div class="drop-zone-marker hidden rounded border p-6 text-center">
				Drop here to add to virtual branch
			</div>
			{#each files as file (file.id)}
				<FileCard
					id={file.id}
					filepath={file.path}
					expanded={file.expanded}
					hunks={file.hunks}
					{dzType}
					{maximized}
					on:expanded={(e) => {
						setExpandedWithCache(file, e.detail);
						expandFromCache();
					}}
					{projectPath}
				/>
			{/each}
			{#if files.length == 0}
				<!-- attention: these markers have custom css at the bottom of thise file -->
				<div
					class="no-changes rounded border border-zinc-200 p-2 text-center dark:border-zinc-700"
					data-dnd-ignore
				>
					No changes made
				</div>
			{/if}
		</div>
	</div>
	<div class="relative">
		<!-- Commit bubble track -->
		<div class="absolute top-0 h-full w-0.5 bg-light-400 dark:bg-dark-500" style="left: 0.925rem" />
		<div class="flex w-full p-2">
			<div class="z-10 w-6" />
			<div class="flex flex-grow gap-x-4 py-2">
				{#if localCommits.length > 0}
					<Button on:click={push} loading={isPushing} color="basic" height="small"
						>Push Commits</Button
					>
				{/if}
			</div>
		</div>
		<!-- Unpushed commits -->
		{#each localCommits as commit (commit.id)}
			<div class="flex w-full px-2 pb-4">
				<div class="z-10 ml-1 w-6 py-4">
					<!-- Unpushed commit bubble -->
					<div
						class="h-2 w-2 rounded-full border-2 border-light-600 bg-light-200 dark:border-dark-200 dark:bg-dark-1000"
					/>
				</div>
				<div class="flex-grow">
					<CommitCard {commit} />
				</div>
			</div>
		{/each}
	</div>
	{#if remoteCommits.length > 0}
		<div class="relative">
			<!-- Commit bubble track -->
			<div class="absolute top-0 h-full w-0.5 bg-light-600" style="left: 0.925rem" />
			<!-- Section title for remote commits -->
			<div class="flex w-full px-2 pb-4">
				<div class="z-10 ml-1 w-6 py-4">
					<div
						class="h-2 w-2 rounded-full border-2 border-light-200 bg-light-200 text-black dark:border-dark-200 dark:bg-dark-200 dark:text-white"
					>
						<!-- Target HEAD commit bubble -->
						<IconGithub />
					</div>
				</div>
				<div class="flex-grow">Pushed to {upstream}</div>
			</div>
			{#each remoteCommits as commit (commit.id)}
				<div class="flex w-full px-2 pb-4">
					<div class="z-10 ml-1 w-6 py-4">
						<!-- Pushed commit bubble -->
						<div
							class="rounded--b-sm h-2 w-2 rounded-full border-2 border-light-200 bg-light-600 dark:border-dark-200 dark:bg-dark-200"
						/>
					</div>
					<CommitCard {commit} />
				</div>
			{/each}
		</div>
	{/if}
</div>
