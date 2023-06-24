<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { Commit, File, Hunk } from './types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { createFile } from './helpers';
	import FileCard from './FileCard.svelte';
	import { invoke } from '@tauri-apps/api';
	import { IconBranch } from '$lib/icons';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import { Button } from '$lib/components';
	import IconMeatballMenu from '$lib/icons/IconMeatballMenu.svelte';
	import CommitCard from './CommitCard.svelte';
	import IconGithub from '$lib/icons/IconGithub.svelte';

	export let branchId: string;
	export let name: string;
	export let commitMessage: string;
	export let files: File[];
	export let commits: Commit[];
	export let projectId: string;

	let allExpanded = true;

	$: remoteCommits = commits.filter((c) => c.isRemote);
	$: localCommits = commits.filter((c) => !c.isRemote);

	let descriptionHeight = 0;
	let textArea: HTMLTextAreaElement;
	const dispatch = createEventDispatcher();

	const move_files = async (params: { projectId: string; branch: string; paths: Array<string> }) =>
		invoke<object>('move_virtual_branch_files', params);

	const commit_branch = async (params: { projectId: string; branch: string; message: string }) =>
		invoke<object>('commit_virtual_branch', params);

	function handleDndEvent(e: CustomEvent<DndEvent<File | Hunk>>) {
		const newItems = e.detail.items;
		const fileItems = newItems.filter((item) => item instanceof File) as File[];

		if (e.type == 'finalize') {
			console.log({
				projectId: projectId,
				branch: branchId,
				paths: fileItems.map((item) => item.path)
			});
			move_files({
				projectId: projectId,
				branch: branchId,
				paths: fileItems.map((item) => item.path)
			});
		}

		const hunkItems = newItems.filter((item) => item instanceof Hunk) as Hunk[];
		for (const hunk of hunkItems) {
			const file = fileItems.find((file) => file.path == hunk.filePath);
			if (file) {
				file.hunks.push(hunk);
			} else {
				fileItems.push(createFile(hunk.filePath, hunk));
			}
		}

		files = fileItems.filter((file) => file.hunks && file.hunks.length > 0);
		if (e.type == 'finalize' && files.length == 0) dispatch('empty');
	}

	function handleEmpty() {
		const emptyIndex = files.findIndex((item) => !item.hunks || item.hunks.length == 0);
		if (emptyIndex != -1) {
			files.splice(emptyIndex, 1);
		}
		if (files.length == 0) {
			dispatch('empty');
		}
		files = files;
	}

	function updateTextArea(): void {
		descriptionHeight = textArea.scrollHeight + 2;
	}

	function commit() {
		console.log('commit', textArea.value, projectId, branchId);
		commit_branch({
			projectId: projectId,
			branch: branchId,
			message: textArea.value
		}).then((res) => {
			console.log(res);
		});
	}

	onMount(() => {
		updateTextArea();
		const hunkCount = files.reduce((acc, cur) => acc + cur.hunks.length, 0);
		if (hunkCount > 10) {
			allExpanded = false;
		}
	});
</script>

<div class="flex max-h-full w-96 shrink-0 flex-col overflow-y-auto  p-4  dark:text-dark-100">
	<div
		class="mb-2 flex w-full shrink-0 items-center rounded-lg bg-light-200 px-3 py-2 text-lg font-bold text-light-900 dark:bg-dark-1000 dark:font-normal dark:text-dark-100"
	>
		<div class="mr-3 flex-grow-0 text-light-600 dark:text-dark-200">
			<IconBranch />
		</div>
		<div class="flex-grow">{name}</div>
		<div class="flex-grow-0 text-light-600 dark:text-dark-200">
			<IconMeatballMenu />
		</div>
	</div>

	<div
		class="flex flex-col rounded-lg bg-white p-2 shadow-lg dark:border dark:border-dark-600 dark:bg-dark-900"
	>
		<div>
			<textarea
				bind:this={textArea}
				class="h-14 w-full shrink-0 resize-none rounded border border-light-200 bg-white p-2 text-dark-800 dark:border-dark-500 dark:bg-dark-700 dark:text-light-400"
				style="height: {descriptionHeight}px"
				value={commitMessage ? commitMessage.trim() : ''}
				placeholder="Your commit message here..."
				on:input={updateTextArea}
			/>
		</div>
		<div class="flex justify-end">
			<button
				class="flex h-6 w-6 items-center justify-center text-light-600 dark:text-dark-200"
				on:click={() => (allExpanded = !allExpanded)}
			>
				{#if allExpanded}
					<IconTriangleUp />
				{:else}
					<IconTriangleDown />
				{/if}
			</button>
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
			{#each files.filter((x) => x.hunks) as file (file.id)}
				<FileCard
					filepath={file.path}
					expanded={allExpanded}
					bind:hunks={file.hunks}
					on:empty={handleEmpty}
				/>
			{/each}
			<Button
				width="full-width"
				color="purple"
				on:click={() => {
					commit();
				}}>Commit</Button
			>
		</div>
	</div>
	<div class="relative">
		<!-- Commit bubble track -->
		<div class="absolute top-0 h-full w-0.5 bg-light-400 dark:bg-dark-500" style="left: 0.925rem" />
		<div class="flex w-full p-2">
			<div class="z-10 w-6" />
			<div class="flex flex-grow gap-x-4 py-2">
				<Button color="basic" height="small">Push Commits</Button>
				<Button color="basic" height="small">Pull Commits</Button>
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
						class="h-4 w-4 rounded-full border-2 border-light-200 bg-light-200 text-white dark:border-dark-200 dark:bg-dark-200 dark:text-black"
					>
						<!-- Target HEAD commit bubble -->
						<IconGithub />
					</div>
				</div>
				<div class="flex-grow">Pushed to origin/master</div>
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
