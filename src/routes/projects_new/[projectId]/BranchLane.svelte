<script lang="ts">
	import { dndzone } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action/typings';
	import { File, Hunk } from './types';
	import { createEventDispatcher, onMount } from 'svelte';
	import { createFile } from './helpers';
	import FileCard from './FileCard.svelte';
	import { invoke } from '@tauri-apps/api';
	import { IconBranch } from '$lib/icons';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import { Button } from '$lib/components';
	import { message } from '@tauri-apps/api/dialog';

	export let branchId: string;
	export let name: string;
	export let commitMessage: string;
	export let files: File[];
	export let projectId: string;

	let allExpanded = true;
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
		descriptionHeight = textArea.scrollHeight;
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
	});
</script>

<div
	class="flex max-h-full w-96 shrink-0 flex-col overflow-y-hidden rounded-xl bg-white px-2 shadow dark:bg-black"
>
	<div class="flex h-16 shrink-0 items-center px-3 text-lg font-bold">
		<IconBranch class="mr-3 text-[#A1A1AA]" />
		{name}
	</div>
	<div
		class="flex flex-col overflow-y-hidden rounded-lg border border-light-700 bg-light-800 p-2 dark:border-dark-700 dark:bg-dark-800"
	>
		<div>
			<textarea
				bind:this={textArea}
				class="mb-5 h-14 w-full shrink-0 resize-none rounded border-0 bg-light-800 py-0 text-dark-400 focus-within:h-36 dark:bg-dark-800 dark:text-light-400"
				style="height: {descriptionHeight}px"
				value={commitMessage ? commitMessage.trim() : ''}
				placeholder="Your commit message here..."
				on:input={updateTextArea}
			/>
			<div class="flex justify-center gap-2">
				<Button disabled={true} width="full-width">Pull</Button>
				<Button disabled={true} width="full-width">Push</Button>
				<Button
					width="full-width"
					color="purple"
					on:click={() => {
						commit();
					}}>Commit</Button
				>
			</div>
		</div>
		<div class="flex justify-end">
			<button
				class="flex h-6 w-6 items-center justify-center"
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
			class="flex flex-shrink flex-col gap-y-2 overflow-y-auto rounded-lg"
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
			<div
				data-dnd-ignore
				class="flex h-full w-full flex-col border-t border-light-200 p-2 dark:border-dark-200"
			>
				<div class="font-bold">Commits</div>
				<div>Commit 1</div>
				<div>Commit 2</div>
				<div>Commit 3</div>
				<div>Commit 1</div>
			</div>
		</div>
	</div>
</div>
