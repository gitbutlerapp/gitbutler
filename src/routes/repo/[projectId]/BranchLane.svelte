<script lang="ts">
	import type { Commit, File, Hunk } from '$lib/api/ipc/vbranches';
	import { createEventDispatcher, onMount } from 'svelte';
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

	const dispatch = createEventDispatcher<{
		empty: never;
	}>();

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

	const hoverClass = 'drag-zone-hover';
	const dzType = 'text/hunk';

	function updateBranchOwnership() {
		const ownership = files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id).join(','))
			.join('\n');
		console.log('updateBranchOwnership', branchId, ownership);
		branchController.updateBranchOwnership(branchId, ownership);
		if (files.length == 0) dispatch('empty');
	}

	function handleFileUpdate(fileId: string, hunks: Hunk[]) {
		const fileIndex = files.findIndex((f) => f.id == fileId);
		if (fileIndex == -1) {
			return;
		} else {
			if (hunks.length === 0) {
				files.splice(fileIndex, 1);
			} else {
				files[fileIndex].hunks = hunks;
			}
			files = files;
			if (files.length === 0) dispatch('empty');
			updateBranchOwnership();
		}
	}

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

	function isChildOf(child: any, parent: HTMLElement): boolean {
		if (parent === child) return false;
		if (!child.parentElement) return false;
		if (child.parentElement == parent) return true;
		return isChildOf(child.parentElement, parent);
	}
</script>

<div
	draggable="true"
	class:w-full={maximized}
	class:w-96={!maximized}
	class="flex max-h-full min-w-[24rem] max-w-[120ch] shrink-0 cursor-grabbing snap-center flex-col overflow-y-auto bg-light-200 py-2 px-3 transition-width dark:bg-dark-1000 dark:text-dark-100"
	use:dzHighlight={{ type: dzType, hover: hoverClass, active: 'drag-zone-active' }}
	on:dragstart
	on:dragend
	on:drop|stopPropagation={(e) => {
		if (!e.dataTransfer) {
			return;
		}
		const data = e.dataTransfer.getData(dzType);
		const ownership = files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id).join(','))
			.join('\n');
		branchController.updateBranchOwnership(branchId, (data + '\n' + ownership).trim());
	}}
>
	<div
		class="mb-2 flex w-full shrink-0 items-center gap-x-2 rounded-lg bg-light-200 text-lg font-bold text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
	>
		<div
			on:dblclick={() => (maximized = !maximized)}
			class="flex-grow-0 cursor-pointer text-light-600 dark:text-dark-200"
		>
			<IconBranch />
		</div>
		<div class="flex-grow">
			<input
				type="text"
				bind:value={name}
				on:change={handleBranchNameChange}
				title={name}
				class="w-full truncate border-0 bg-light-200 text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
			/>
		</div>
		<button
			bind:this={meatballButton}
			class="flex-grow-0 p-2 text-light-600 dark:text-dark-200"
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

		<PopupMenuItem on:click={() => branchController.createBranch({ order: order + 1 })}>
			Create branch after
		</PopupMenuItem>

		<PopupMenuItem on:click={() => branchController.createBranch({ order })}>
			Create branch before
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
		<div class="mb-4 text-right">
			<Button
				height="small"
				color="purple"
				on:click={() => {
					commit();
				}}>Commit</Button
			>
		</div>
		<div class="flex flex-shrink flex-col gap-y-2">
			<div class="drag-zone-marker hidden rounded-lg border p-6">
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
				<div class="no-changes p-2" data-dnd-ignore>No uncommitted work on this branch.</div>
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
				<div class="z-10 w-6 py-2">
					<!-- Unpushed commit bubble -->
					<div
						class="h-4 w-4 rounded-full border-2 border-light-600 bg-light-200 dark:border-dark-200 dark:bg-dark-1000"
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
				<div class="z-10 w-6">
					<div
						class="h-4 w-4 rounded-full border-2 border-light-200 bg-light-200 text-black dark:border-dark-200 dark:bg-dark-200 dark:text-white"
					>
						<!-- Target HEAD commit bubble -->
						<IconGithub />
					</div>
				</div>
				<div class="flex-grow">Pushed to {upstream}</div>
			</div>
			{#each remoteCommits as commit (commit.id)}
				<div class="flex w-full px-2 pb-4">
					<div class="z-10 w-6 py-2">
						<!-- Pushed commit bubble -->
						<div
							class="rounded--b-sm h-4 w-4 rounded-full border-2 border-light-200 bg-light-600 dark:border-dark-200 dark:bg-dark-200"
						/>
					</div>
					<CommitCard {commit} />
				</div>
			{/each}
		</div>
	{/if}
</div>
