<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import type { Readable } from 'svelte/store';
	import { currentProject } from '$lib/current_project';
	import { IconCircleCancel } from '$lib/components/icons';
	import type { Project } from '$lib/projects';
	import tinykeys from 'tinykeys';
	import type { CommandGroup, Command } from './commands';
	import { Action, previousCommand, nextCommand, firstVisibleCommand } from './commands';
	import type { ComponentType } from 'svelte';
	import { default as RewindCommand } from './RewindCommand.svelte';
	import { default as HelpCommand } from './HelpCommand.svelte';
	import { invoke } from '@tauri-apps/api';

	const matchFiles = (params: { projectId: string; matchPattern: string }) =>
		invoke<Array<string>>('git_match_paths', params);

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

	$: scopeToProject = $currentProject ? true : false;

	let showingCommandPalette = false;
	let dialog: HTMLDialogElement;
	let userInput: string;

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

	let componentOfTriggeredCommand: ComponentType | undefined;
	let triggeredCommand: Command | undefined;

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
					title: 'Replay History',
					description: 'Command',
					selected: false,
					action: {
						component: RewindCommand
					},
					visible: 'replay'.includes(userInput?.toLowerCase())
				},
				{
					title: 'Help',
					description: 'Command',
					selected: false,
					action: {
						component: HelpCommand
					},
					visible: 'help'.includes(userInput?.toLowerCase())
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

	const resetState = () => {
		userInput = '';
		scopeToProject = $currentProject ? true : false;
		selection = [0, 0];
		componentOfTriggeredCommand = undefined;
		triggeredCommand = undefined;
		matchingFiles = [];
	};

	const triggerCommand = () => {
		if (
			!commandGroups[selection[0]].visible ||
			!commandGroups[selection[0]].commands[selection[1]].visible
		) {
			return;
		}
		if (Action.isLink(selectedCommand.action)) {
			toggleCommandPalette();
			goto(selectedCommand.action.href);
		} else if (Action.isActionInPalette(selectedCommand.action)) {
			userInput = '';
			componentOfTriggeredCommand = selectedCommand.action.component;
			triggeredCommand = selectedCommand;
		}
	};

	const toggleCommandPalette = () => {
		if (dialog && dialog.open) {
			dialog.close();
			showingCommandPalette = false;
		} else {
			resetState();
			dialog.showModal();
			showingCommandPalette = true;
		}
	};

	let unsubscribeKeyboardHandler: () => void;

	onMount(() => {
		toggleCommandPalette(); // developmnet only
		unsubscribeKeyboardHandler = tinykeys(window, {
			'Meta+k': () => {
				toggleCommandPalette();
			},
			Backspace: () => {
				if (!userInput) {
					if (triggeredCommand) {
						// Untrigger command
						componentOfTriggeredCommand = undefined;
						triggeredCommand = undefined;
					} else {
						scopeToProject = false;
					}
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

<!-- svelte-ignore a11y-click-events-have-key-events -->
<dialog
	class="rounded-lg 
    border border-zinc-400/40
    bg-zinc-900/70 p-0 backdrop-blur-xl
    "
	bind:this={dialog}
	on:click|self={() => toggleCommandPalette()}
>
	<div class="flex h-[640px] w-[640px] flex-col rounded text-zinc-400" on:click|stopPropagation>
		<!-- Search input area -->
		<div class="flex items-center border-b border-zinc-400/20 py-2">
			<div class="ml-4 mr-2 flex flex-grow items-center">
				<!-- Project scope -->
				{#if scopeToProject}
					<div class="mr-1 flex items-center">
						<span class="font-semibold text-zinc-300">{$currentProject?.title}</span>
						<span class="ml-1 text-lg">/</span>
					</div>
				{/if}
				<!-- Selected command -->
				{#if scopeToProject && triggeredCommand}
					<div class="mr-1 flex items-center">
						<span class="font-semibold text-zinc-300">{triggeredCommand?.title}</span>
						<span class="ml-1 text-lg">/</span>
					</div>
				{/if}
				<!-- Search input -->
				<div class="mr-1 flex-grow">
					<!-- svelte-ignore a11y-autofocus -->
					<input
						class="w-full bg-transparent text-zinc-300 focus:outline-none"
						bind:value={userInput}
						on:input={updateMatchFilesQuery}
						type="text"
						autofocus
						placeholder={!scopeToProject
							? 'Search for repositories'
							: !componentOfTriggeredCommand
							? 'Search for commands, files and code changes...'
							: ''}
					/>
				</div>
				<button on:click={toggleCommandPalette} class="rounded p-2 hover:bg-zinc-600">
					<IconCircleCancel class="fill-zinc-400" />
				</button>
			</div>
		</div>
		<!-- Main part -->
		<div class="flex-auto overflow-y-auto">
			{#if componentOfTriggeredCommand}
				<svelte:component this={componentOfTriggeredCommand} {userInput} />
			{:else}
				{#each commandGroups as group, groupIdx}
					{#if group.visible}
						<div class="mx-2 cursor-default select-none">
							<p
								class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300/80"
							>
								<span>{group.name}</span>
								{#if group.description}
									<span class="ml-2 font-light italic text-zinc-300/60">({group.description})</span>
								{/if}
							</p>
							<ul class="">
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
												<span class="flex-grow">{command.title}</span>
												<span>{command.description}</span>
											</a>
										{:else if Action.isActionInPalette(command.action)}
											<div
												on:mouseover={() => (selection = [groupIdx, commandIdx])}
												on:focus={() => (selection = [groupIdx, commandIdx])}
												on:click={triggerCommand}
												class="{selection[0] === groupIdx && selection[1] === commandIdx
													? 'bg-zinc-700/70'
													: ''} flex cursor-default items-center rounded-lg p-2 px-2 outline-none"
											>
												<span class="flex-grow">{command.title}</span>
												<span>{command.description}</span>
											</div>
										{/if}
									{/if}
								{/each}
							</ul>
						</div>
					{/if}
				{/each}
			{/if}
		</div>
	</div>
</dialog>
