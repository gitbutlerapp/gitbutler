<script lang="ts">
	import { fade } from 'svelte/transition';
	import FileIcon from './icons/FileIcon.svelte';
	import CommitIcon from './icons/CommitIcon.svelte';
	import BookmarkIcon from './icons/BookmarkIcon.svelte';
	import BranchIcon from './icons/BranchIcon.svelte';
	import ContactIcon from './icons/ContactIcon.svelte';
	import ProjectIcon from './icons/ProjectIcon.svelte';
	import { invoke } from '@tauri-apps/api';
	import { goto } from '$app/navigation';
	import { shortPath } from '$lib/paths';
	import { currentProject } from '$lib/current_project';
	import type { Project } from '$lib/projects';

	let showCommand = false;
	let showCommit = false;

	let is_command_down = false;
	let is_k_down = false;
	let is_c_down = false;
	let is_e_down = false;

	let palette: HTMLElement;
	let commitPalette: HTMLElement;

	let changedFiles = {};
	let commitMessage = '';
	let commitMessageInput: HTMLElement;

	const listFiles = (params: { projectId: string }) =>
		invoke<Record<string, string>>('git_status', params);

	const matchFiles = (params: { projectId: string; matchPattern: string }) =>
		invoke<Array<string>>('git_match_paths', params);

	const listProjects = () => invoke<Project[]>('list_projects');

	const commit = (params: {
		projectId: string;
		message: string;
		files: Array<string>;
		push: boolean;
	}) => invoke<boolean>('git_commit', params);

	function onKeyDown(event: KeyboardEvent) {
		if (event.repeat) return;
		switch (event.key) {
			case 'Meta':
				is_command_down = true;
				event.preventDefault();
				break;
			case 'k':
				is_k_down = true;
				break;
			case 'c':
				is_c_down = true;
				break;
			case 'e':
				is_e_down = true;
				break;
			case 'Escape':
				showCommand = false;
				showCommit = false;
				break;
			case 'ArrowDown':
				if (showCommand) {
					event.preventDefault();
					downMenu();
				}
				break;
			case 'ArrowUp':
				if (showCommand) {
					event.preventDefault();
					upMenu();
				}
				break;
			case 'Enter':
				if (showCommand) {
					event.preventDefault();
					selectItem();
				}
				break;
		}
		if (is_command_down && is_k_down) {
			showCommand = true;
			setTimeout(function () {
				document.getElementById('command')?.focus();
			}, 100);
		}
		if (is_command_down && is_c_down) {
			showCommit = true;
			executeCommand('commit');
		}
		if (is_command_down && is_e_down) {
			executeCommand('contact');
		}
	}

	function onKeyUp(event: KeyboardEvent) {
		switch (event.key) {
			case 'Meta':
				is_command_down = false;
				event.preventDefault();
				break;
			case 'k':
				is_k_down = false;
				event.preventDefault();
				break;
			case 'c':
				is_c_down = false;
				event.preventDefault();
				break;
			case 'e':
				is_e_down = false;
				event.preventDefault();
				break;
		}
	}

	function checkCommandModal(event: Event) {
		const target = event.target as HTMLElement;
		if (showCommand && !palette.contains(target)) {
			showCommand = false;
		}
		if (showCommit && !commitPalette.contains(target)) {
			showCommit = false;
		}
	}

	let activeClass = ['active', 'bg-zinc-700/50', 'text-white'];

	function upMenu() {
		const menu = document.getElementById('commandMenu');
		if (menu) {
			const items = menu.querySelectorAll('li.item');
			const active = menu.querySelector('li.active');
			if (active) {
				const index = Array.from(items).indexOf(active);
				if (index > 0) {
					items[index - 1].classList.add(...activeClass);
				}
				active.classList.remove(...activeClass);
			} else {
				items[items.length - 1].classList.add(...activeClass);
			}
		}
	}

	function downMenu() {
		const menu = document.getElementById('commandMenu');
		if (menu) {
			const items = menu.querySelectorAll('li.item');
			const active = menu.querySelector('li.active');
			if (active) {
				const index = Array.from(items).indexOf(active);
				if (index < items.length - 1) {
					items[index + 1].classList.add(...activeClass);
					active.classList.remove(...activeClass);
				}
			} else {
				items[0].classList.add(...activeClass);
			}
		}
	}

	function selectItem() {
		showCommand = false;
		showCommit = false;
		const menu = document.getElementById('commandMenu');
		if (menu) {
			const active = menu.querySelector('li.active');
			if (active) {
				const command = active.getAttribute('data-command');
				const context = active.getAttribute('data-context');
				if (command) {
					executeCommand(command, context);
				}
			} else {
				if ($currentProject) {
					goto('/projects/' + $currentProject.id + '/search?search=' + search);
				}
			}
		}
	}

	function executeCommand(command: string, context?: string | null) {
		switch (command) {
			case 'commit':
				if ($currentProject) {
					listFiles({ projectId: $currentProject.id }).then((files) => {
						console.log('files', files);
						changedFiles = files;
					});
					showCommit = true;
					setTimeout(function () {
						commitMessageInput.focus();
					}, 100);
				}
				break;
			case 'contact':
				console.log('contact us');
				goto('/contact');
				break;
			case 'switch':
				console.log('switch', command, context);
				goto('/projects/' + context);
				break;
			case 'bookmark':
				break;
			case 'branch':
				break;
		}
	}

	let search = '';

	$: {
		searchChanged(search, showCommand);
	}

	let projectCommands = [
		{ text: 'Commit', key: 'C', icon: CommitIcon, command: 'commit' },
		{ text: 'Bookmark', key: 'B', icon: BookmarkIcon, command: 'bookmark' },
		{ text: 'Branch', key: 'R', icon: BranchIcon, command: 'branch' }
	];

	let switchCommands = [];
	$: if ($currentProject) {
		listProjects().then((projects) => {
			switchCommands = [];
			projects.forEach((p) => {
				if (p.id !== $currentProject?.id) {
					switchCommands.push({
						text: p.title,
						icon: ProjectIcon,
						command: 'switch',
						context: p.id
					});
				}
			});
		});
	}

	let baseCommands = [{ text: 'Contact Us', key: 'E', icon: ContactIcon, command: 'contact' }];

	function commandList() {
		let commands = [];
		let divider = [{ type: 'divider' }];
		if ($currentProject) {
			commands = projectCommands.concat(divider).concat(switchCommands);
		} else {
			commands = switchCommands;
		}
		commands = commands.concat(divider).concat(baseCommands);
		return commands;
	}

	$: menuItems = commandList();

	function searchChanged(searchValue: string, showCommand: boolean) {
		if (!showCommand) {
			search = '';
		}
		if (searchValue.length == 0) {
			updateMenu([]);
			return;
		}
		if ($currentProject) {
			const searchPattern = '.*' + Array.from(searchValue).join('(.*)');
			matchFiles({ projectId: $currentProject.id, matchPattern: searchPattern }).then((files) => {
				let searchResults = [];
				files.slice(0, 5).forEach((f) => {
					searchResults.push({ text: f, icon: FileIcon });
				});
				updateMenu(searchResults);
			});
		}
	}

	function updateMenu(searchResults: Array<{ text: string }>) {
		if (searchResults.length == 0) {
			menuItems = commandList();
		} else {
			menuItems = searchResults;
		}
	}

	function doCommit() {
		// get checked files
		let changedFiles: Array<string> = [];
		let doc = document.getElementsByClassName('file-checkbox');
		Array.from(doc).forEach((c) => {
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
				console.log('commit result', result);
				commitMessage = '';
				showCommit = false;
			});
		}
	}
</script>

<svelte:window on:keydown={onKeyDown} on:keyup={onKeyUp} on:click={checkCommandModal} />

<div>
	{#if showCommand || showCommit}
		<div class="relative z-10" role="dialog" aria-modal="true">
			<div
				class="fixed inset-0 bg-zinc-900 bg-opacity-80 transition-opacity"
				in:fade={{ duration: 50 }}
				out:fade={{ duration: 50 }}
			/>

			{#if showCommand}
				<div class="command-palette-modal fixed inset-0 z-10 overflow-y-auto p-4 sm:p-6 md:p-20">
					<div
						bind:this={palette}
						in:fade={{ duration: 100 }}
						out:fade={{ duration: 100 }}
						class="mx-auto max-w-2xl transform divide-y divide-zinc-500 divide-opacity-20 overflow-hidden rounded-xl bg-zinc-900 shadow-2xl transition-all border border-zinc-700"
						style="
							height: auto;
							max-height: 420px;
							border-width: 0.5px; 
							-webkit-backdrop-filter: blur(20px) saturate(190%) contrast(70%) brightness(80%);
							backdrop-filter: blur(20px) saturate(190%) contrast(70%) brightness(80%);
							background-color: rgba(24, 24, 27, 0.60);
							border: 0.5px solid rgba(63, 63, 70, 0.50);"
					>
						<div class="relative">
							<svg
								class="pointer-events-none absolute top-3.5 left-4 h-5 w-5 text-zinc-500"
								fill="none"
								viewBox="0 0 24 24"
								stroke-width="1.5"
								stroke="currentColor"
								aria-hidden="true"
							>
								<path
									stroke-linecap="round"
									stroke-linejoin="round"
									d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
								/>
							</svg>
							<input
								id="command"
								type="text"
								bind:value={search}
								class="h-12 w-full border-0 bg-transparent pl-11 pr-4 text-white focus:ring-0 sm:text-sm"
								placeholder="Search..."
							/>
						</div>

						<!-- Default state, show/hide based on command palette state. -->
						<ul
							class="scroll-py-2 divide-y divide-zinc-500 divide-opacity-20 overflow-y-auto"
						>
							<li class="p-1">
								<ul id="commandMenu" class="text-sm text-zinc-400">
									{#each menuItems as item}
										{#if item.type == 'divider'}
											<li class="border-t border-zinc-500 border-opacity-20 my-2" />
										{:else}
											<!-- Active: "bg-zinc-800 text-white" -->
											<li
												class="item group flex cursor-default select-none items-center rounded-md px-3 py-2"
												on:click={() => {
													executeCommand(item.command);
												}}
												data-command={item.command}
												data-context={item.context}
											>
												<!-- Active: "text-white", Not Active: "text-zinc-500" -->
												<svelte:component this={item.icon} />
												<span class="ml-3 flex-auto truncate">{item.text}</span>
												{#if item.key}
													<span class="ml-3 flex-none text-xs font-semibold text-zinc-400 px-1 py-1 bg-zinc-800 border-b border-black rounded">
														<kbd class="font-sans">⌘</kbd><kbd class="font-sans">{item.key}</kbd>
													</span>
												{/if}
											</li>
										{/if}
									{/each}
								</ul>
							</li>
						</ul>
					</div>
				</div>
			{/if}

			{#if showCommit}
				<div class="commit-palette-modal fixed inset-0 z-10 overflow-y-auto p-4 sm:p-6 md:p-20">
					<div
						in:fade={{ duration: 100 }}
						out:fade={{ duration: 100 }}
						bind:this={commitPalette}
						class="mx-auto max-w-2xl transform overflow-hidden rounded-xl bg-zinc-900 shadow-2xl transition-all border border-zinc-700"
						style="
							border-width: 0.5px; 
							border: 0.5px solid rgba(63, 63, 70, 0.50);
							-webkit-backdrop-filter: blur(20px) saturate(190%) contrast(70%) brightness(80%); 
							background-color: rgba(24, 24, 27, 0.6);
							"
					>
						<div class="w-full border-b border-zinc-700 text-lg text-white mb-4 p-4">
							Commit Your Changes
						</div>
						<div
							class="relative transform overflow-hidden text-left transition-all sm:w-full sm:max-w-sm p-2 m-auto"
						>
							{#if Object.entries(changedFiles).length > 0}
								<div>
									<div class="">
										<h3 class="text-base font-semibold text-zinc-200" id="modal-title">
											Commit Message
										</h3>
										<div class="mt-2">
											<div class="mt-2">
												<textarea
													rows="4"
													name="message"
													id="commit-message"
													bind:this={commitMessageInput}
													bind:value={commitMessage}
													class="block w-full rounded-md p-4 border-0 text-zinc-200 ring-1 ring-inset ring-gray-300 placeholder:text-gray-400 focus:ring-2 focus:ring-inset focus:ring-blue-600 sm:py-1.5 sm:text-sm sm:leading-6"
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
								<div class="text-zinc-200 mt-4 py-4">
									<h3 class="text-base font-semibold text-zinc-200" id="modal-title">
										Changed Files
									</h3>
									{#each Object.entries(changedFiles) as file}
										<div class="flex flex-row space-x-2">
											<div>
												<input type="checkbox" class="file-checkbox" data-file={file[0]} checked />
											</div>
											<div>
												{file[1]}
											</div>
											<div class="font-mono">
												{shortPath(file[0])}
											</div>
										</div>
									{/each}
								</div>
							{:else}
								<div class="text-white mx-auto text-center">No changes to commit</div>
							{/if}
						</div>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>
