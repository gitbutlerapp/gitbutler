<script lang="ts">
	import { fade } from 'svelte/transition';
	import FileIcon from './icons/FileIcon.svelte';
	import CommitIcon from './icons/CommitIcon.svelte';
	import BookmarkIcon from './icons/BookmarkIcon.svelte';
	import BranchIcon from './icons/BranchIcon.svelte';
	import { invoke } from '@tauri-apps/api';
	import { redirect } from '@sveltejs/kit';
	import { goto } from '$app/navigation';

	let showCommand = false;
	let is_command_down = false;
	let is_k_down = false;

	export let projectId: string;

	let palette: HTMLElement;

	const matchFiles = (params: { projectId: string; matchPattern: string }) =>
		invoke<Array<string>>('git_match_paths', params);

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
			case 'Escape':
				showCommand = false;
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
		}
	}

	function checkCommandModal(event: Event) {
		const target = event.target as HTMLElement;
		if (showCommand && !palette.contains(target)) {
			showCommand = false;
		}
	}

	let activeClass = ['active', 'bg-zinc-700/50', 'text-white'];

	function upMenu() {
		const menu = document.getElementById('commandMenu');
		if (menu) {
			const items = menu.querySelectorAll('li');
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
		console.log('DOWN');
		const menu = document.getElementById('commandMenu');
		console.log('menu', menu);
		if (menu) {
			const items = menu.querySelectorAll('li');
			console.log('items', items);
			const active = menu.querySelector('li.active');
			console.log('active', active);
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
		const menu = document.getElementById('commandMenu');
		if (menu) {
			const active = menu.querySelector('li.active');
			if (active) {
				console.log('active', active);
			} else {
				goto('/projects/' + projectId + '/search?search=' + search);
			}
		}
	}

	let search = '';

	$: {
		searchChanged(search, showCommand);
	}

	let baseCommands = [
		{ text: 'Commit', key: 'C', icon: CommitIcon },
		{ text: 'Bookmark', key: 'B', icon: BookmarkIcon },
		{ text: 'Branch', key: 'H', icon: BranchIcon }
	];

	$: menuItems = baseCommands;

	function searchChanged(searchValue: string, showCommand: boolean) {
		if (!showCommand) {
			search = '';
		}
		if (searchValue.length == 0) {
			updateMenu([]);
			return;
		}
		const searchPattern = '.*' + Array.from(searchValue).join('(.*)');
		matchFiles({ projectId: projectId, matchPattern: searchPattern }).then((files) => {
			let searchResults = [];
			files.slice(0, 5).forEach((f) => {
				searchResults.push({ text: f, icon: FileIcon });
			});
			updateMenu(searchResults);
		});
	}

	function updateMenu(searchResults: Array<{ text: string }>) {
		if (searchResults.length == 0) {
			menuItems = baseCommands;
		} else {
			menuItems = searchResults;
		}
	}
</script>

<svelte:window on:keydown={onKeyDown} on:keyup={onKeyUp} on:click={checkCommandModal} />

<div>
	{#if showCommand}
		<div class="relative z-10" role="dialog" aria-modal="true">
			<div
				class="fixed inset-0 bg-zinc-500 bg-opacity-25 transition-opacity"
				in:fade={{ duration: 50 }}
				out:fade={{ duration: 50 }}
			/>

			<div class="fixed inset-0 z-10 overflow-y-auto p-4 sm:p-6 md:p-20">
				<div
					bind:this={palette}
					in:fade={{ duration: 100 }}
					out:fade={{ duration: 100 }}
					class="mx-auto max-w-2xl transform divide-y divide-zinc-500 divide-opacity-20 overflow-hidden rounded-xl bg-zinc-900 shadow-2xl transition-all border border-zinc-700"
					style="border-width: 0.5px;"
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
						class="max-h-80 scroll-py-2 divide-y divide-zinc-500 divide-opacity-20 overflow-y-auto"
					>
						<li class="p-1">
							<ul id="commandMenu" class="text-sm text-zinc-400">
								{#each menuItems as item}
									<!-- Active: "bg-zinc-800 text-white" -->
									<li
										class="group flex cursor-default select-none items-center rounded-md px-3 py-2"
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
								{/each}
							</ul>
						</li>
					</ul>
				</div>
			</div>
		</div>
	{/if}
</div>
