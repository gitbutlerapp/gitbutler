<script lang="ts">
	import Modal from '../Modal.svelte';
	import { currentProject } from '$lib/current_project';
	import { getContext } from 'svelte';
	import type { Readable } from 'svelte/store';
	import type { Project } from '$lib/projects';
	import type { CommandGroup, Command } from './types';
	import { onDestroy, onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';
	import { Action, previousCommand, nextCommand, firstVisibleCommand } from './types';
	import Replay from './Replay.svelte';
	import Commit from './Commit.svelte';
	import { invoke } from '@tauri-apps/api';
	import { createEventDispatcher } from 'svelte';
	import { RewindIcon } from '$lib/components/icons';

	const dispatch = createEventDispatcher();

	$: scopeToProject = $currentProject ? true : false;

	let userInput = '';

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};
	let matchFilesQuery = '';
	const updateMatchFilesQuery = debounce(async () => {
		matchFilesQuery = userInput;
	}, 100);
	const matchFiles = (params: { projectId: string; matchPattern: string }) =>
		invoke<Array<string>>('git_match_paths', params);
	let matchingFiles: Array<string> = [];
	$: if (matchFilesQuery) {
		matchFiles({ projectId: $currentProject?.id || '', matchPattern: matchFilesQuery }).then(
			(files) => {
				matchingFiles = files;
			}
		);
	} else {
		matchingFiles = [];
	}

	let projects: Readable<any> = getContext('projects');

	let selection: [number, number] = [0, 0];
	// if the group or the command are no longer visible, select the first visible group and first visible command
	$: if (
		!commandGroups[selection[0]]?.visible ||
		!commandGroups[selection[0]].commands[selection[1]]?.visible
	) {
		selection = firstVisibleCommand(commandGroups);
	}
	$: selectedCommand = commandGroups[selection[0]].commands[selection[1]];
	$: {
		const element = document.getElementById(`${selection[0]}-${selection[1]}`);
		if (element) {
			// TODO: this works, but it's not standard
			element.scrollIntoViewIfNeeded(false);
		}
	}

	$: commandGroups = [
		{
			name: 'Go to project',
			visible: !scopeToProject,
			commands: $projects.map((project: Project) => {
				return {
					title: project.title,
					description: 'Repository',
					selected: false,
					action: {
						href: `/projects/${project.id}/`
					},
					visible: project.title.toLowerCase().includes(userInput?.toLowerCase())
				};
			})
		},
		{
			name: 'Actions',
			visible: scopeToProject,
			commands: [
				{
					title: 'Commit',
					description: 'C',
					selected: false,
					action: {
						component: Commit
					},
					icon: RewindIcon,
					visible: 'commit'.includes(userInput?.toLowerCase())
				},
				{
					title: 'Replay History',
					description: 'R',
					selected: false,
					action: {
						component: Replay
					},
					icon: RewindIcon,
					visible: 'replay history'.includes(userInput?.toLowerCase())
				}
			]
		},
		{
			name: 'Files',
			visible: scopeToProject,
			description: !userInput
				? 'type part of a file name'
				: matchingFiles.length === 0
				? `no files containing '${userInput}'`
				: '',
			commands: matchingFiles.map((file) => {
				return {
					title: file,
					description: 'File',
					selected: false,
					action: {
						href: `/`
					},
					visible: true
				};
			})
		}
	] as CommandGroup[];

	const triggerCommand = () => {
		// If the selected command is a link, navigate to it, otherwise, emit a 'newdialog' event, handled in the parent component
		dispatch('newdialog', Commit);
		if (Action.isLink(selectedCommand.action)) {
			goto(selectedCommand.action.href);
			dispatch('close');
		} else if (Action.isActionInPalette(selectedCommand.action)) {
			dispatch('newdialog', selectedCommand.action.component);
		}
	};

	let unsubscribeKeyboardHandler: () => void;

	onMount(() => {
		unsubscribeKeyboardHandler = tinykeys(window, {
			Backspace: () => {
				if (!userInput) {
					scopeToProject = false;
				}
			},
			Enter: () => {
				triggerCommand();
			},
			ArrowDown: () => {
				selection = nextCommand(commandGroups, selection);
			},
			ArrowUp: () => {
				selection = previousCommand(commandGroups, selection);
			},
			'Control+n': () => {
				selection = nextCommand(commandGroups, selection);
			},
			'Control+p': () => {
				selection = previousCommand(commandGroups, selection);
			}
		});
	});

	onDestroy(() => {
		unsubscribeKeyboardHandler?.();
	});
</script>

<Modal on:close>
	<!-- svelte-ignore a11y-click-events-have-key-events -->
	<div class="commnand-pallete flex h-[640px] w-[640px] flex-col rounded text-zinc-400" on:click|stopPropagation>
		<!-- Search input area -->
		<div class="search-input flex items-center border-b border-zinc-400/20 py-2">
			<div class="ml-4 mr-2 flex flex-grow items-center">
				<!-- Project scope -->
				{#if scopeToProject}
					<div class="mr-1 flex items-center">
						<span class="font-semibold text-zinc-300">{$currentProject?.title}</span>
						<span class="ml-1 text-lg">/</span>
					</div>
				{/if}
				<!-- Search input -->
				<div class="mr-1 flex-grow">
					<!-- svelte-ignore a11y-autofocus -->
					<input
						class="w-full bg-transparent text-zinc-300 focus:outline-none"
						bind:value={userInput}
						on:input|stopPropagation={updateMatchFilesQuery}
						type="text"
						autofocus
						placeholder={!scopeToProject
							? 'Search for repositories'
							: 'Search for commands, files and code changes...'}
					/>
				</div>
			</div>
		</div>
		<!-- Main part -->
		<div class="search-results h-[640px] flex-auto overflow-y-auto">
			{#each commandGroups as group, groupIdx}
				{#if group.visible}
					<div class="mx-2 cursor-default select-none">
						<p class="result-section-header mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300">
							<span>{group.name}</span>
							{#if group.description}
								<span class="ml-2 font-light italic text-zinc-300/70">({group.description})</span>
							{/if}
						</p>
						<ul class="section-list text-zinc-300">
							{#each group.commands as command, commandIdx}
								{#if command.visible}
									{#if Action.isLink(command.action)}
										<a
											on:mouseover={() => (selection = [groupIdx, commandIdx])}
											on:focus={() => (selection = [groupIdx, commandIdx])}
											id={`${groupIdx}-${commandIdx}`}
											href={command.action.href}
											class="{selection[0] === groupIdx && selection[1] === commandIdx
												? 'bg-zinc-700/70'
												: ''} flex cursor-default items-center rounded-lg p-2 px-2 outline-none"
										>
											<span class="quick-command flex-grow">{command.title}</span>
											<span class="quick-command-key">{command.description}</span>
										</a>
									{:else if Action.isActionInPalette(command.action)}
										<div
											on:mouseover={() => (selection = [groupIdx, commandIdx])}
											on:focus={() => (selection = [groupIdx, commandIdx])}
											on:click={triggerCommand}
											class="{selection[0] === groupIdx && selection[1] === commandIdx
												? 'bg-zinc-50/10'
												: ''} flex cursor-default items-center rounded-lg p-2 px-2 outline-none gap-2"
										>
											<span class="quick-command-icon">
												<svelte:component this={command.icon} />
											</span>
											<span class="quick-command flex-grow">{command.title}</span>
											<span class="quick-command-key ">{command.description}</span>
										</div>
									{/if}
								{/if}
							{/each}
						</ul>
					</div>
				{/if}
			{/each}
		</div>
	</div>
</Modal>
