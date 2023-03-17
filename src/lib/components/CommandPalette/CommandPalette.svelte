<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { getContext } from 'svelte';
	import type { Readable } from 'svelte/store';
	import { currentProject } from '$lib/current_project';
	import { IconCircleCancel } from '$lib/components/icons';
	import type { Project } from '$lib/projects';
	import tinykeys from 'tinykeys';
	import type { CommandGroup } from './commands';
	import { Action, previousCommand, nextCommand, firstVisibleCommand } from './commands';

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

	$: commandGroups = [
		{
			name: 'Repositories',
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
		}
	] as CommandGroup[];

	const resetState = () => {
		userInput = '';
		scopeToProject = $currentProject ? true : false;
		selection = [0, 0];
	};

	const handleEnter = () => {
		if (!commandGroups[0].visible || !commandGroups[0].commands[0].visible) {
			return;
		}
		const command = commandGroups[selection[0]].commands[selection[1]];
		if (Action.isLink(command.action)) {
			toggleCommandPalette();
			goto(command.action.href);
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
		// toggleCommandPalette(); // developmnet only
		unsubscribeKeyboardHandler = tinykeys(window, {
			'Meta+k': () => {
				toggleCommandPalette();
			},
			Backspace: () => {
				if (!userInput) {
					scopeToProject = false;
				}
			},
			Enter: () => {
				handleEnter();
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
	<div class="min-h-[640px] w-[640px] rounded text-zinc-400" on:click|stopPropagation>
		<!-- Search input area -->
		<div class="flex h-14 items-center border-b border-zinc-400/20">
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
						type="text"
						autofocus
						placeholder={scopeToProject
							? 'Search for commands, files and code changes...'
							: 'Search for repositories'}
					/>
				</div>
				<button on:click={toggleCommandPalette} class="rounded p-2 hover:bg-zinc-600">
					<IconCircleCancel class="fill-zinc-400" />
				</button>
			</div>
		</div>
		<!-- Main part -->
		<div>
			{#each commandGroups as group, groupIdx}
				{#if group.visible}
					<div class="mx-2 cursor-default select-none">
						<p class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300/80">
							{group.name}
						</p>
						<ul class="">
							{#each group.commands as command, commandIdx}
								{#if command.visible}
									{#if Action.isLink(command.action)}
										<a
											on:mouseover={() => (selection = [groupIdx, commandIdx])}
											on:focus={() => (selection = [groupIdx, commandIdx])}
											href={command.action.href}
											class="{selection[0] === groupIdx && selection[1] === commandIdx
												? 'bg-zinc-700/70'
												: ''} flex cursor-default items-center rounded-lg p-2 px-2 outline-none"
										>
											<span class="flex-grow">{command.title}</span>
											<span>{command.description}</span>
										</a>
									{/if}
								{/if}
							{/each}
						</ul>
					</div>
				{/if}
			{/each}
		</div>
	</div>
</dialog>
