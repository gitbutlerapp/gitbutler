<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { Commit, File, Hunk } from './types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { createFile } from './helpers';
	import FileCard from './FileCard.svelte';
	import { IconBranch } from '$lib/icons';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import { Button } from '$lib/components';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';
	import { getExpandedWithCacheFallback, setExpandedWithCache } from './cache';
	import type { VirtualBranchOperations } from './vbranches';

	const dispatch = createEventDispatcher<{
		empty: never;
	}>();

	export let branchId: string;
	export let name: string;
	export let commitMessage: string;
	export let upstream: string;
	export let files: File[];
	export let commits: Commit[];
	export let projectId: string;
	export let virtualBranches: VirtualBranchOperations;

	$: remoteCommits = commits.filter((c) => c.isRemote);
	$: localCommits = commits.filter((c) => !c.isRemote);

	let allExpanded: boolean | undefined;
	let descriptionHeight = 0;
	let textArea: HTMLTextAreaElement;
	let isPushing = false;

	function handleDndEvent(e: CustomEvent<DndEvent<File | Hunk>>) {
		const newItems = e.detail.items;
		const fileItems = newItems.filter((item) => item instanceof File) as File[];

		console.log('lane: handleDndEvent', e.type, e.detail.items);

		const hunkItems = newItems.filter((item) => item instanceof Hunk) as Hunk[];
		hunkItems.forEach((hunk) => {
			const file = files.find((f) => f.hunks.find((h) => h.id == hunk.id));
			if (file) {
				file.hunks.push(hunk);
			} else {
				fileItems.push(createFile(hunk.filePath, hunk));
			}
		});

		files = fileItems.filter((file) => file.hunks && file.hunks.length > 0);
		if (e.type === 'finalize') updateBranchOwnership();
	}

	function updateBranchOwnership() {
		const ownership = files
			.map((file) => file.id + ':' + file.hunks.map((hunk) => hunk.id.split(':')[1]).join(','))
			.join('\n');
		console.log('updateBranchOwnership', branchId, ownership);
		virtualBranches.updateBranchOwnership(branchId, ownership);
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

	function updateTextArea(): void {
		if (textArea) {
			descriptionHeight = textArea.scrollHeight + 2;
		}
	}

	function commit() {
		console.log('commit', textArea.value, projectId, branchId);
		virtualBranches.commitBranch(branchId, textArea.value);
	}

	function push() {
		if (localCommits[0]?.id) {
			console.log(`pushing ${branchId}`);
			isPushing = true;
			virtualBranches.pushBranch(branchId).finally(() => (isPushing = false));
		}
	}

	onMount(() => {
		updateTextArea();
		expandFromCache();
	});

	$: {
		// On refresh we need to check expansion status from localStorage
		files && expandFromCache();
		updateTextArea();
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
		virtualBranches.updateBranchName(branchId, name);
	}
</script>

<div class="flex max-h-full w-[22.5rem] shrink-0 flex-col overflow-y-auto  p-4  dark:text-dark-100">
	<div
		class="mb-2 flex w-full shrink-0 items-center rounded-lg bg-light-200 px-3 py-2 text-lg font-bold text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
	>
		<div class="mr-3 flex-grow-0 text-light-600 dark:text-dark-200">
			<IconBranch />
		</div>
		<div class="flex-grow">
			<input
				type="text"
				bind:value={name}
				on:change={handleBranchNameChange}
				class="border-0 bg-light-200 text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
			/>
		</div>
		<div class="ml-3 flex-grow-0 text-light-600 dark:text-dark-200">
			<IconMeatballMenu />
		</div>
	</div>

	<div
		class="flex flex-col rounded-lg bg-white p-2 shadow-lg dark:border dark:border-dark-600 dark:bg-dark-900"
	>
		<div class="mb-4 flex items-center">
			{#if files.filter((x) => x.hunks).length > 0}
				<textarea
					bind:this={textArea}
					class="h-14 shrink-0 flex-grow resize-none rounded border border-light-200 bg-white p-2 text-dark-800 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400"
					style="height: {descriptionHeight}px"
					value={commitMessage ? commitMessage.trim() : ''}
					placeholder="Your commit message here..."
					on:input={updateTextArea}
				/>
				<button
					class="mx-0.5 h-6 w-6 items-center justify-center text-light-600 dark:text-dark-200"
					on:click={handleToggleExpandAll}
				>
					{#if allExpanded}
						<IconTriangleUp />
					{:else if allExpanded == undefined}
						<IconTriangleDown />
					{:else}
						<IconTriangleDown />
					{/if}
				</button>
			{/if}
		</div>
		<div
			class="flex flex-shrink flex-col gap-y-2"
			use:dndzone={{
				items: files,
				zoneTabIndex: -1,
				types: ['file'],
				receives: ['file', 'hunk']
			}}
			on:consider={handleDndEvent}
			on:finalize={handleDndEvent}
		>
			<Button
				width="basic"
				kind="outlined"
				height="small"
				color="destructive"
				on:click={() => {
					virtualBranches.deleteBranch(branchId);
				}}>delete</Button
			>
			{#each files.filter((x) => x.hunks) as file (file.id)}
				<FileCard
					filepath={file.path}
					expanded={file.expanded}
					hunks={file.hunks}
					on:update={(e) => {
						handleFileUpdate(file.id, e.detail);
					}}
					on:expanded={(e) => {
						setExpandedWithCache(file, e.detail);
						expandFromCache();
					}}
				/>
			{/each}
			{#if files.filter((x) => x.hunks).length > 0}
				<Button
					width="full-width"
					color="purple"
					on:click={() => {
						commit();
					}}>Commit</Button
				>
			{:else}
				<div class="p-3 pt-0">No uncommitted work on this branch.</div>
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
