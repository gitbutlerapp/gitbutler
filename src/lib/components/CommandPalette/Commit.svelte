<script lang="ts">
	import Modal from '../Modal.svelte';
	import { collapsable } from '$lib/paths';
	import { invoke } from '@tauri-apps/api';
	import { currentProject } from '$lib/current_project';
	import { onMount } from 'svelte';
	import toast from 'svelte-french-toast';
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher();

	let commitMessage = '';

	let changedFiles: Record<string, string> = {};

	const listFiles = (params: { projectId: string }) =>
		invoke<Record<string, string>>('git_status', params);

	const commit = (params: {
		projectId: string;
		message: string;
		files: Array<string>;
		push: boolean;
	}) => invoke<boolean>('git_commit', params);

	let modal: Modal;
	onMount(() => {
		modal.show();
		listFiles({ projectId: $currentProject?.id || '' }).then((files) => {
			changedFiles = files;
		});
	});

	function doCommit() {
		// get checked files
		let changedFiles: Array<string> = [];
		let doc = document.getElementsByClassName('file-checkbox');
		Array.from(doc).forEach((c: any) => {
			if (c.checked) {
				changedFiles.push(c.dataset['file']);
			}
		});
		if ($currentProject) {
			commit({
				projectId: $currentProject.id,
				message: commitMessage,
				files: changedFiles,
				push: false
			}).then((result) => {
				toast.success('Commit successful!', {
					icon: '🎉'
				});
				commitMessage = '';
				dispatch('close');
			});
		}
	}
</script>

<Modal on:close bind:this={modal}>
	<!-- svelte-ignore a11y-click-events-have-key-events -->
	<div class="flex flex-col rounded text-zinc-400" on:click|stopPropagation>
		<div class="mb-4 w-full border-b border-zinc-700 p-4 text-lg text-white">
			Commit Your Changes
		</div>
		<div
			class="relative mx-auto transform overflow-hidden p-2 text-left transition-all sm:w-full sm:max-w-sm"
		>
			{#if Object.entries(changedFiles).length > 0}
				<div>
					<div class="">
						<h3 class="text-base font-semibold text-zinc-200" id="modal-title">Commit Message</h3>
						<div class="mt-2">
							<div class="mt-2">
								<textarea
									rows="4"
									name="message"
									placeholder="Description of changes"
									id="commit-message"
									bind:value={commitMessage}
									class="ring-gray-600 block w-full rounded-md border-0 p-4 text-zinc-200 ring-1 ring-inset placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-1.5 sm:text-sm sm:leading-6"
								/>
							</div>
						</div>
					</div>
				</div>
				<div class="mt-5 sm:mt-6">
					<button
						type="button"
						on:click={doCommit}
						class="inline-flex w-full justify-center rounded-md bg-blue-600 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-blue-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-600"
						>Commit Your Changes</button
					>
				</div>
				<div class="mt-4 py-4 text-zinc-200">
					<h3 class="text-base font-semibold text-zinc-200" id="modal-title">Changed Files</h3>
					{#each Object.entries(changedFiles) as file}
						<div class="flex flex-row space-x-2">
							<div>
								<input type="checkbox" class="file-checkbox" data-file={file[0]} checked />
							</div>
							<div>
								{file[1]}
							</div>
							<span class="font-mono" use:collapsable={{ value: file[0], separator: '/' }} />
						</div>
					{/each}
				</div>
			{:else}
				<div class="mx-auto text-center text-white">No changes to commit</div>
			{/if}
		</div>
	</div>
</Modal>
