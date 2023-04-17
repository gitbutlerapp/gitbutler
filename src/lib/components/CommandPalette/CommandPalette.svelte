<script lang="ts">
	import tinykeys from 'tinykeys';
	import type { Project } from '$lib/projects';
	import { derived, readable, writable, type Readable } from 'svelte/store';
	import { Modal } from '$lib/components';
	import listAvailableCommands, { Action, type Group, type ActionComponent } from './commands';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { open } from '@tauri-apps/api/shell';

	export let projects: Readable<Project[]>;
	export let project = readable<Project | undefined>(undefined);

	const input = writable('');
	const scopeToProject = writable(!!$project);
	project.subscribe((project) => scopeToProject.set(!!project));
	const selectedGroup = writable<Group | undefined>(undefined);
	const selectedComponent = writable<ActionComponent<any> | undefined>(undefined);

	const commandGroups = derived(
		[projects, project, input, scopeToProject, selectedGroup],
		([projects, project, input, scopeToProject, selectedGroup]) =>
			selectedGroup !== undefined
				? [selectedGroup]
				: listAvailableCommands({ projects, project: scopeToProject ? project : undefined, input })
	);

	let selection = [0, 0] as [number, number];
	commandGroups.subscribe((groups) => {
		const newGroupIndex = Math.min(selection[0], groups.length - 1);
		Promise.resolve(groups[newGroupIndex]).then((group) => {
			const newCommandIndex = Math.min(selection[1], group.commands.length - 1);
			selection = [newGroupIndex, newCommandIndex];
		});
	});

	const selectNextCommand = () => {
		if (!modal?.isOpen()) return;
		Promise.resolve($commandGroups[selection[0]]).then((group) => {
			if (selection[1] < group.commands.length - 1) {
				selection = [selection[0], selection[1] + 1];
			} else if (selection[0] < $commandGroups.length - 1) {
				selection = [selection[0] + 1, 0];
			}
		});
	};

	const selectPreviousCommand = () => {
		if (!modal?.isOpen()) return;
		if (selection[1] > 0) {
			selection = [selection[0], selection[1] - 1];
		} else if (selection[0] > 0) {
			Promise.resolve($commandGroups[selection[0] - 1]).then((previousGroup) => {
				selection = [selection[0] - 1, previousGroup.commands.length - 1];
			});
		}
	};

	const trigger = (action: Action) => {
		if (!modal?.isOpen()) return;
		if (Action.isLink(action)) {
			action.href.startsWith('http') || action.href.startsWith('mailto')
				? open(action.href)
				: goto(action.href);
			modal?.hide();
		} else if (Action.isGroup(action)) {
			selectedGroup.set(action);
		} else if (Action.isComponent(action)) {
			selectedComponent.set(action);
		}
		scopeToProject.set(!!$project);
	};

	let modal: Modal | null;

	export const show = () => {
		modal?.show();
	};

	onMount(() =>
		tinykeys(window, {
			Backspace: () => {
				if (!modal?.isOpen()) return;
				if ($selectedGroup) {
					selectedGroup.set(undefined);
				} else if ($selectedComponent) {
					selectedComponent.set(undefined);
				} else if ($input.length === 0) {
					scopeToProject.set(false);
				}
			},
			ArrowDown: selectNextCommand,
			ArrowUp: selectPreviousCommand,
			'Control+n': selectNextCommand,
			'Control+p': selectPreviousCommand,
			Enter: () => {
				if (!modal?.isOpen()) return;
				Promise.resolve($commandGroups[selection[0]]).then((group) =>
					trigger(group.commands[selection[1]].action)
				);
			}
		})
	);

	let unregisterCommandHotkeys: (() => void)[] = [];
	$: {
		unregisterCommandHotkeys.forEach((unregister) => unregister());
		unregisterCommandHotkeys = [];
		commandGroups.subscribe((groups) =>
			groups.forEach((group) =>
				Promise.resolve(group).then((group) =>
					group.commands.forEach((command) => {
						if (command.hotkey) {
							unregisterCommandHotkeys.push(
								tinykeys(window, {
									[command.hotkey]: (event: KeyboardEvent) => {
										const target = event.target as HTMLElement;
										if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA') return;
										// only trigger if the modal is visible
										modal?.isOpen() && trigger(command.action);
									}
								})
							);
						}
					})
				)
			)
		);
	}
</script>

<Modal bind:this={modal}>
	<div
		class="command-palette flex max-h-[400px] min-h-[40px] w-[640px] flex-col rounded text-zinc-400"
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
				{:else if $selectedComponent}
					<span class="font-semibold">
						{$selectedComponent.title}
					</span>
				{:else}
					<!-- svelte-ignore a11y-autofocus -->
					<input
						spellcheck="false"
						class="command-palette-input-field"
						bind:value={$input}
						type="text"
						autofocus
						placeholder={!$project
							? 'Search for repositories'
							: 'Search for commands, files and code changes...'}
					/>
				{/if}
			</div>
		</header>

		{#if $selectedComponent}
			<svelte:component this={$selectedComponent.component} {...$selectedComponent.props} />
		{:else}
			<!-- Command list -->
			<ul class="command-pallete-content-container flex-auto overflow-y-auto pb-2">
				{#each $commandGroups as group, groupIdx}
					{#await group then group}
						<li class="w-full cursor-default select-none px-2">
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
										class:selected={selection[0] === groupIdx && selection[1] === commandIdx}
									>
										<button
											on:mouseover={() => (selection = [groupIdx, commandIdx])}
											on:focus={() => (selection = [groupIdx, commandIdx])}
											on:click={() => trigger(command.action)}
											class="text-color-500 flex w-full items-center gap-2 rounded-lg p-2 px-2  outline-none"
										>
											<svelte:component this={command.icon} class="icon h-5 w-5 text-zinc-500 " />
											<span class="quick-command flex-1 text-left">{command.title}</span>
											{#if command.hotkey}
												{#each command.hotkey.split('+') as key}
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
		{/if}
	</div>
</Modal>

<style lang="postcss">
	.selected {
		@apply bg-zinc-50/10;
	}
	.selected .quick-command {
		@apply text-zinc-100;
	}
	.selected svg .icon {
		border: 1px solid orange !important;
	}
	.command-palette-input-field {
		@apply flex-1 border-0 bg-transparent p-2 outline-none focus:outline-none active:outline-none;
	}
	.command-palette-input-field textarea,
	input {
		@apply focus:border-0 focus:outline-none focus:ring-0;
	}
	.command-palette-section-header {
		@apply mx-2 mb-2 mt-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300;
	}
	/* .quick-command-item {
		@apply gap-2 rounded-lg p-2 px-2 outline-none;
	} */
	.quick-command-key {
		@apply rounded-sm border border-[#3A393F] bg-[#343338] px-[3px] font-mono text-[11px] shadow;
	}
</style>
