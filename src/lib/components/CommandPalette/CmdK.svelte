<script lang="ts">
	import Modal from '../Modal.svelte';
	import type { Readable } from 'svelte/store';
	import type { Project } from '$lib/projects';
	import type { ActionInPalette, CommandGroup } from './types';
	import { onDestroy, onMount } from 'svelte';
	import tinykeys from 'tinykeys';
	import { goto } from '$app/navigation';
	import { Action, previousCommand, nextCommand, firstVisibleCommand } from './types';
	import Replay from './Replay.svelte';
	import Commit from './Commit.svelte';
	import { invoke } from '@tauri-apps/api';
	import { createEventDispatcher } from 'svelte';
	import { RewindIcon } from '$lib/components/icons';
	import { GitCommitIcon } from '$lib/components/icons';

	export let projects: Readable<Project[]>;
	export let project: Readable<Project | undefined>;

	const dispatch = createEventDispatcher<{
		close: void;
		newdialog: ActionInPalette<Commit | Replay>;
	}>();

	$: scopeToProject = $project ? true : false;

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
		matchFiles({ projectId: $project?.id || '', matchPattern: matchFilesQuery }).then((files) => {
			matchingFiles = files;
		});
	} else {
		matchingFiles = [];
	}

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
					title: 'Quick commit',
					description: 'C',
					selected: false,
					action: {
						component: Commit,
						props: { project }
					},
					icon: GitCommitIcon,
					visible: 'commit'.includes(userInput?.toLowerCase())
				},
				{
					title: 'Commit',
					description: 'Shift C',
					selected: false,
					action: {
						href: `/projects/${$project?.id}/commit`
					},
					icon: GitCommitIcon,
					visible: 'commit'.includes(userInput?.toLowerCase())
				},
				{
					title: 'Replay History',
					description: 'R',
					selected: false,
					action: {
						component: Replay,
						props: { project }
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
		if (Action.isLink(selectedCommand.action)) {
			goto(selectedCommand.action.href);
			dispatch('close');
		} else if (Action.isActionInPalette(selectedCommand.action)) {
			dispatch('newdialog', selectedCommand.action);
		}
	};

	let unsubscribeKeyboardHandler: () => void;

	let modal: Modal;
	onMount(() => {
		modal.show();
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

<Modal on:close bind:this={modal}>
	<div class="commnand-palette flex max-h-[360px] w-[640px] flex-col rounded text-zinc-400">
		<!-- Search input area -->
		<div class="search-input flex items-center border-b border-zinc-400/20 py-2">
			<div class="ml-4 mr-2 flex flex-grow items-center">
				<!-- Project scope -->
				{#if scopeToProject}
					<div class="mr-1 flex items-center">
						<span class="font-semibold text-zinc-300">{$project?.title}</span>
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
		<div class="command-pallete-content-container flex-auto overflow-y-auto pb-2">
			{#each commandGroups as group, groupIdx}
				{#if group.visible}
					<div class="px-2 cursor-default select-none w-full">
						<p class="commnand-palette-section-header result-section-header">
							<span>{group.name}</span>
							{#if group.description}
								<span class="ml-2 font-light italic text-zinc-300/70">({group.description})</span>
							{/if}
						</p>
						<ul class="quick-command-list flex flex-col text-zinc-300">
							{#each group.commands as command, commandIdx}
								{#if command.visible}
									<li
										class="{selection[0] === groupIdx && selection[1] === commandIdx
											? 'bg-zinc-50/10'
											: ''} quick-command-item flex w-full cursor-default"
									>
										<button
											on:mouseover={() => (selection = [groupIdx, commandIdx])}
											on:focus={() => (selection = [groupIdx, commandIdx])}
											on:click={triggerCommand}
											class="flex w-full gap-2"
										>
											<svelte:component this={command.icon} />
											<span class="quick-command flex-1 text-left">{command.title}</span>
											<span class="quick-command-key">{command.description}</span>
										</button>
									</li>
								{/if}
							{/each}
						</ul>
					</div>
				{/if}
			{/each}
		</div>
	</div>
</Modal>
