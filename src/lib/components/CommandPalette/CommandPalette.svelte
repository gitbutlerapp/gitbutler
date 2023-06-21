<script lang="ts">
	import tinykeys from 'tinykeys';
	import type { Project } from '$lib/api';
	import { derived, readable, writable, type Readable } from '@square/svelte-store';
	import { Overlay } from '$lib/components';
	import listAvailableCommands, { Action, type Group } from './commands';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { open } from '@tauri-apps/api/shell';
	import { IconExternalLink } from '../../icons';

	export let projects: Readable<Project[]>;
	export let project = readable<Project | undefined>(undefined);

	const input = writable('');
	const scopeToProject = writable(!!$project);
	project.subscribe((project) => scopeToProject.set(!!project));
	const selectedGroup = writable<Group | undefined>(undefined);

	const commandGroups = derived(
		[projects, project, input, scopeToProject, selectedGroup],
		([projects, project, input, scopeToProject, selectedGroup]) =>
			selectedGroup !== undefined
				? [selectedGroup]
				: listAvailableCommands({
						projects,
						project: scopeToProject ? project : undefined,
						input
				  })
	);

	const visibleEntriesCount = derived([commandGroups], async ([commandGroups]) => {
		const sizes = await Promise.all(
			commandGroups.map(async (group) => {
				const g = await group;
				return g.commands.length;
			})
		);
		return sizes.reduce((acc, size) => acc + size, 0);
	});

	const selection = writable<[number, number]>([0, 0]);

	commandGroups.subscribe((groups) => {
		const newGroupIndex = Math.min($selection[0], groups.length - 1);
		Promise.resolve(groups[newGroupIndex]).then((group) => {
			const newCommandIndex = Math.min($selection[1], group.commands.length - 1);
			$selection = [newGroupIndex, newCommandIndex];
		});
	});

	selection.subscribe(() => {
		const selected = document.querySelector('.selected');
		if (selected) {
			selected.scrollIntoView({ block: 'center', behavior: 'smooth' });
		}
	});

	const selectNextCommand = async () => {
		if (!modal?.isOpen()) return;
		const group = await Promise.resolve($commandGroups[$selection[0]]);
		const nextCommandIndex = group.commands.findIndex((_command, index) => index > $selection[1]);
		if (nextCommandIndex > -1) {
			$selection = [$selection[0], nextCommandIndex];
		} else {
			await selectNextGroup();
		}
	};

	const selectNextGroup = async () => {
		if (!modal?.isOpen()) return;
		const groups = await Promise.all($commandGroups.map((group) => Promise.resolve(group)));
		const nextGroupIndex = groups.findIndex(
			(group, index) => index > $selection[0] && group.commands.length > 0
		);
		if (nextGroupIndex > -1) {
			$selection = [nextGroupIndex, 0];
		}
	};

	const selectPreviousCommand = async () => {
		if (!modal?.isOpen()) return;
		const group = await Promise.resolve($commandGroups[$selection[0]]);
		const previousCommandIndex = group.commands
			.map((_command, index) => index < $selection[1])
			.lastIndexOf(true);
		if (previousCommandIndex > -1) {
			$selection = [$selection[0], previousCommandIndex];
		} else {
			await selectPreviousGroup();
		}
	};

	const selectPreviousGroup = async () => {
		if (!modal?.isOpen()) return;
		const groups = await Promise.all($commandGroups.map((group) => Promise.resolve(group)));
		const previousGroupIndex = groups
			.map((group, index) => index < $selection[0] && group.commands.length > 0)
			.lastIndexOf(true);
		if (previousGroupIndex > -1) {
			$selection = [previousGroupIndex, groups[previousGroupIndex].commands.length - 1];
		}
	};

	const trigger = (action: Action) => {
		if (!modal?.isOpen()) return;
		if (Action.isLink(action)) {
			action.href.startsWith('http') || action.href.startsWith('mailto')
				? open(action.href)
				: goto(action.href);
			modal?.close();
		} else if (Action.isGroup(action)) {
			selectedGroup.set(action);
		} else if (Action.isRun(action)) {
			action();
			modal?.close();
		}
		scopeToProject.set(!!$project);
	};

	let modal: Overlay | null;

	const reset = () => {
		input.set('');
		scopeToProject.set(!!$project);
		selectedGroup.set(undefined);
		$selection = [0, 0];
	};

	export function show() {
		reset();
		modal?.show();
	}

	export function close() {
		modal?.close();
	}

	onMount(() =>
		tinykeys(window, {
			Backspace: () => {
				if (!modal?.isOpen()) return;

				if ($selectedGroup) {
					selectedGroup.set(undefined);
				} else if ($input.length === 0) {
					scopeToProject.set(false);
				}
			},
			ArrowDown: (event) => {
				if (!modal?.isOpen()) return;
				event.preventDefault();
				event.stopPropagation();
				selectNextCommand();
			},
			ArrowUp: (event) => {
				if (!modal?.isOpen()) return;
				event.preventDefault();
				event.stopPropagation();
				selectPreviousCommand();
			},
			'Control+n': (event) => {
				if (!modal?.isOpen()) return;
				event.preventDefault();
				event.stopPropagation();
				selectNextCommand();
			},
			'Control+p': (event) => {
				if (!modal?.isOpen()) return;
				event.preventDefault();
				event.stopPropagation();
				selectPreviousCommand();
			},
			Enter: (event) => {
				if (!modal?.isOpen()) return;
				event.preventDefault();
				event.stopPropagation();
				Promise.resolve($commandGroups[$selection[0]]).then((group) =>
					trigger(group.commands[$selection[1]].action)
				);
			}
		})
	);
</script>

<Overlay bind:this={modal}>
	<div class="h-[400px]">
		<div
			class="command-palette modal modal-command-palette flex max-h-[400px] min-h-[40px] w-[680px] flex-col "
		>
			<!-- Search input area -->
			<header class="search-input-container flex items-center border-b border-zinc-400/20 py-2">
				<div class="ml-4 mr-2 flex w-full items-center gap-1 text-lg text-zinc-300">
					<!-- Project scope -->
					{#if $scopeToProject && $project}
						<span class="py-2 font-semibold">
							{$project.title}
						</span>
						<span>/</span>
					{/if}
					{#if $selectedGroup}
						<span class="font-semibold">
							{$selectedGroup.title}
						</span>
					{:else}
						<!-- svelte-ignore a11y-autofocus -->
						<input
							spellcheck="true"
							autocomplete="off"
							autocorrect="off"
							class="command-palette-input-field"
							bind:value={$input}
							type="text"
							autofocus
							placeholder={!$project
								? 'Search your projects'
								: 'Search for commands, files and code changes'}
						/>
					{/if}
				</div>
			</header>

			<!-- Command list -->
			<ul class="command-pallete-content-container flex-auto overflow-y-auto">
				{#each $commandGroups as group, groupIdx}
					{#await group then group}
						<li
							class="my-2 w-full cursor-default select-none px-2 "
							class:hidden={group.commands.length === 0}
						>
							<header class="command-palette-section-header result-section-header">
								<span>{group.title}</span>
								{#if group.description}
									<span class="ml-2 font-light italic text-zinc-300/70">({group.description})</span>
								{/if}
							</header>

							<ul class="quick-command-list flex flex-col text-zinc-300">
								{#each group.commands as command, commandIdx}
									<li
										class="quick-command-item flex w-full cursor-default rounded-lg"
										class:selected={$selection[0] === groupIdx && $selection[1] === commandIdx}
									>
										<button
											on:focus={() => ($selection = [groupIdx, commandIdx])}
											on:click={() => trigger(command.action)}
											class="text-color-500 flex w-full items-center gap-2 rounded-lg p-2 px-2  outline-none"
										>
											<svelte:component this={command.icon} class="icon h-5 w-5 text-zinc-500 " />
											<span
												class="quick-command flex flex-1 items-center gap-1 text-left font-medium"
											>
												{command.title}
												{#if Action.isExternalLink(command.action)}
													<IconExternalLink class="h-4 w-4 text-zinc-600" />
												{/if}
											</span>
											{#if command.hotkey}
												{#each command.hotkey.replace('Meta', '⌘').split('+') as key}
													<span class="quick-command-key">{key}</span>
												{/each}
											{/if}
										</button>
									</li>
								{/each}
							</ul>
						</li>
					{/await}
				{/each}
			</ul>
			{#await $visibleEntriesCount then resultsCount}
				{#if resultsCount === 0}
					<div class="flex flex-1 flex-col items-center justify-center">
						<span class="w-full px-4 py-2.5 font-semibold leading-8 text-zinc-300"
							>Nothing turned up. Try again?</span
						>
					</div>
				{/if}
			{/await}
		</div>
	</div>
</Overlay>

<style lang="postcss">
	.command-palette {
		@apply rounded-lg border-[0.5px] border-[#3F3F3f]  p-0 text-zinc-400 shadow-lg backdrop-blur-lg;
		/* @apply bg-zinc-900/70;		 */
	}
	.command-pallete-content-container {
		/* @apply pt-2; */
	}
	.quick-command-item:hover {
		@apply rounded-sm bg-zinc-50/5;
	}

	.selected,
	.selected:hover {
		@apply bg-zinc-50/10;
	}

	.selected .quick-command {
		@apply text-zinc-100;
	}
	.command-palette-input-field {
		@apply flex-1 border-0 bg-transparent p-2 outline-none focus:outline-none active:outline-none;
		outline: none;
	}
	.command-palette-input-field:focus {
		outline: 0;
		outline-offset: 0;
		box-shadow: rgb(255, 255, 255) 0px 0px 0px 0px, rgba(37, 99, 235, 0) 0px 0px 0px 2px,
			rgba(0, 0, 0, 0) 0px 0px 0px 0px;
	}
	.command-palette-section-header {
		@apply mx-2 mb-2 cursor-default select-none pt-2 text-sm font-semibold text-zinc-400;
	}
	.quick-command-key {
		@apply rounded-sm border border-[#3A393F] bg-[#343338] px-[3px] font-mono text-[11px] shadow;
	}
</style>
