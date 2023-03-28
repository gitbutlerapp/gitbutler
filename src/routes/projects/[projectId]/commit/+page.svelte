<script lang="ts">
	import { invoke } from '@tauri-apps/api';
	import type { PageData } from './$types';
	import Api from '$lib/api';
	import { collapsable } from '$lib/paths';
	import toast from 'svelte-french-toast';
	import { slide } from 'svelte/transition';
	import { toHumanBranchName } from '$lib/branch';
	import DiffViewer from '$lib/components/DiffViewer.svelte';

	const api = Api({ fetch });

	export let data: PageData;
	const { project, user, filesStatus } = data;

	let commitSubject: string;
	let placeholderSubject = 'Summary (required)';
	let commitMessage: string;
	let placeholderMessage = 'Optional description of changes';
	let messageRows = 6;
	let filesSelectedForCommit: string[] = [];

	const commit = (params: {
		projectId: string;
		message: string;
		files: Array<string>;
		push: boolean;
	}) => invoke<boolean>('git_commit', params);

	function doCommit() {
		if ($project) {
			if (commitMessage) {
				commitSubject = commitSubject + '\n\n' + commitMessage;
			}
			commit({
				projectId: $project.id,
				message: commitSubject,
				files: filesSelectedForCommit,
				push: false
			}).then((result) => {
				toast.success('Commit successful!', {
					icon: '🎉'
				});
				commitMessage = '';
				commitSubject = '';
				filesSelectedForCommit = [];
				currentDiff = '';
				currentPath = '';
				isLoaded = false;
			});
		}
	}

	const toggleAllOff = () => {
		filesSelectedForCommit = [];
	};
	const toggleAllOn = () => {
		filesSelectedForCommit = $filesStatus.map((file) => {
			return file.path;
		});
	};
	const showMessage = (message: string) => {
		generatedMessage = undefined;
	};

	const getDiff = (params: { projectId: string }) =>
		invoke<Record<string, string>>('git_wd_diff', params);
	const getBranch = (params: { projectId: string }) => invoke<string>('git_branch', params);
	const getFile = (params: { projectId: string; path: string }) =>
		invoke<string>('get_file_contents', params);

	let gitBranch: string | undefined = undefined;
	let gitDiff: Record<string, string> = {};
	let generatedMessage: string | undefined = undefined;
	let isLoaded = false;

	let currentPath = '';
	let currentDiff = '';
	let fileContents = '';
	let fileContentsStatus = '';

	// Replace HTML tags with an empty string
	function selectPath(path: string) {
		currentDiff = '';
		fileContents = '';

		if (gitDiff[path]) {
			currentPath = path;
			currentDiff = gitDiff[path];
		} else {
			let file = $filesStatus.filter((file) => file.path === path)[0];
			if ($project && file) {
				fileContentsStatus = file.status;
				getFile({ projectId: $project.id, path: path }).then((contents) => {
					currentPath = path;
					fileContents = contents;
				});
			}
		}
	}

	$: if ($project) {
		if (!isLoaded) {
			getBranch({ projectId: $project?.id }).then((branch) => {
				gitBranch = branch;
				filesSelectedForCommit = $filesStatus.map((file) => {
					return file.path;
				});
			});
			getDiff({ projectId: $project?.id }).then((diff) => {
				gitDiff = diff;
			});
			isLoaded = true;
		}
	}

	let loadingPercent = 0;
	function fetchCommitMessage() {
		if ($project && $user) {
			// make diff from keys of gitDiff matching entries in filesSelectedForCommit
			const partialDiff = Object.fromEntries(
				Object.entries(gitDiff).filter(([key]) => filesSelectedForCommit.includes(key))
			);
			// convert to string
			const diff = Object.values(partialDiff).join('\n').slice(0, 5000); // limit for summary

			placeholderMessage = 'Summarizing changes...';
			generatedMessage = 'loading';
			loadingPercent = 0;
			// every second update loadingPercent by 8%
			const interval = setInterval(() => {
				loadingPercent += 6.25;
				if (loadingPercent >= 100) {
					clearInterval(interval);
				}
			}, 1000);

			api.summarize
				.commit($user?.access_token, {
					diff: diff,
					uid: $project.id
				})
				.then((message) => {
					if (message) {
						// split result into subject and message (first line is subject)
						commitSubject = message.split('\n')[0];
						commitMessage = message.split('\n').slice(2).join('\n');
						generatedMessage = message;
						// set messageRows as a function of the number of chars in the message
						messageRows = Math.ceil(commitMessage.length / 75) + 3;
					}
					loadingPercent = 100;
				});
		}
	}
</script>

<div class="flex h-full flex-row">
	<div class="flex w-[500px] min-w-[500px] flex-shrink-0 flex-col p-2">
		<div
			class="button group mb-2 flex max-w-[500px] rounded border border-zinc-600 bg-zinc-700 py-2 px-4 text-zinc-300 shadow"
		>
			<div class="h-4 w-4">
				<svg
					aria-hidden="true"
					height="16"
					viewBox="0 0 16 16"
					version="1.1"
					width="16"
					data-view-component="true"
					class="h-4 w-4 fill-zinc-400"
				>
					<path
						d="M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.493 2.493 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Zm-6 0a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Zm8.25-.75a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z"
					/>
				</svg>
			</div>
			<div class="truncate pl-2 font-mono text-zinc-300">
				{toHumanBranchName(gitBranch)}
			</div>
			<div class="carrot flex hidden items-center pl-3">
				<svg width="7" height="5" viewBox="0 0 7 5" fill="none" class="fill-zinc-400">
					<path
						d="M3.87796 4.56356C3.67858 4.79379 3.32142 4.79379 3.12204 4.56356L0.319371 1.32733C0.0389327 1.00351 0.268959 0.5 0.697336 0.5L6.30267 0.500001C6.73104 0.500001 6.96107 1.00351 6.68063 1.32733L3.87796 4.56356Z"
						fill="#A1A1AA"
					/>
				</svg>
			</div>
		</div>

		<div class="changed-files-list-container mt-2 mb-4">
			<div
				class="changed-files-list flex flex-col rounded border border-[0.5px] border-gb-700 bg-gb-900 font-mono text-zinc-900"
			>
				<div
					class="flex flex-row space-x-2 justify-between rounded-t border-b border-b-gb-750 bg-gb-800 p-2 text-zinc-200"
				>
					<h3 class="text-base font-semibold">Changed files</h3>
					<div>
						<button 
							title="Select all"
							class="text-yellow-200" 
							on:click={toggleAllOn}>
							all
						</button>
						<button 
							title="Deselect all"
							class="text-yellow-200" 
							on:click={toggleAllOff}>
							none
						</button>
					</div>
				</div>
				<ul class="truncate px-2 py-2 min-h-[35px]">
					{#each $filesStatus as activity}
						<li class="list-none text-zinc-300">
							<div class="flex flex-row align-middle">
								<input
									type="checkbox"
									on:click={() => showMessage}
									bind:group={filesSelectedForCommit}
									value={activity.path}
									class="mr-2 mt-1 w-4"
								/>
								<div class="w-4">{activity.status.slice(0, 1)}</div>
								<button
									title="{activity.path}"
									class="text-left w-[100%] cursor-pointer truncate {currentPath == activity.path
										? 'text-white'
										: ''}"
									on:click={() => selectPath(activity.path)}
									use:collapsable={{ value: activity.path, separator: '/' }}
								/>
							</div>
						</li>
					{/each}
				</ul>
			</div>
		</div>

		<div class="commit-input-container" transition:slide={{ duration: 150 }}>
			<h3 class="mb-2 text-base font-semibold text-zinc-300">Commit Message</h3>
			<input
				type="text"
				name="subject"
				bind:value={commitSubject}
				placeholder={placeholderSubject}
				class="mb-2 block w-full rounded-md border-zinc-600 bg-zinc-700 p-4 text-zinc-200 ring-1 ring-inset ring-gray-600 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-3 sm:text-sm sm:leading-4"
			/>
			<textarea
				rows={messageRows}
				name="message"
				placeholder={placeholderMessage}
				bind:value={commitMessage}
				class="mb-2 block w-full rounded-md border-zinc-600 bg-zinc-700 p-4 text-zinc-200 ring-1 ring-inset ring-gray-600 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-3 sm:text-sm sm:leading-4"
			/>
		</div>

		<div class="flex flex-row justify-between">
			{#if filesSelectedForCommit.length == 0}
				<div>Select at least one file.</div>
			{:else if !commitSubject}
				<div>Provide a commit message.</div>
			{:else}
				<button
					disabled={!commitSubject || filesSelectedForCommit.length == 0}
					class="{!commitSubject || filesSelectedForCommit.length == 0
						? 'bg-zinc-800 text-zinc-600'
						: ''} button rounded bg-blue-600 py-2 px-3 text-white"
					on:click={() => {
						doCommit();
					}}>Commit changes</button
				>
			{/if}
			{#if !generatedMessage}

				<a
					title="Generate commit message"
					class="cursor-pointer rounded bg-green-800 bg-gradient-to-b from-[#623871] to-[#502E5C] p-2 text-zinc-50 shadow"
					on:click={fetchCommitMessage}>
					✨ Generate commit message
				</a>
			{:else if generatedMessage == 'loading'}
				<div class="flex flex-col">
					<div class="text-zinc-400">Let me take a look at these changes...</div>
					<!-- status bar filled by loadingPercent -->
					<div class="h-2.5 w-full rounded-full bg-gray-200 dark:bg-gray-700">
						<div
							class="h-2.5 rounded-full bg-green-600"
							style="width: {Math.round(loadingPercent)}%"
						/>
					</div>
				</div>
			{/if}
		</div>
	</div>
	<div class="h-100 h-full max-h-screen flex-grow overflow-auto p-2">
		{#if currentDiff}
			<DiffViewer diff={currentDiff} path={currentPath} />
		{:else if fileContents}
			<pre
				class={fileContentsStatus == 'added' ? 'bg-green-900' : 'bg-red-900'}>{fileContents}
			</pre>
		{:else}
			<div class="p-20 text-center text-lg text-zinc-400">Select a file to view changes.</div>
		{/if}
	</div>
	<!-- commit message -->
</div>
