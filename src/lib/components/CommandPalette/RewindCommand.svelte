<script lang="ts">
	import type { Command } from './commands';
	import { Action, previousCommand, nextCommand, firstVisibleCommand } from './commands';
	import tinykeys from 'tinykeys';
	import { onDestroy, onMount } from 'svelte';

	let unsubscribeKeyboardHandler: () => void;

	onMount(() => {
		unsubscribeKeyboardHandler = tinykeys(window, {
			ArrowDown: () => {
				selection = nextSubCommand(innerCommands, selection);
			},
			ArrowUp: () => {
				selection = previousSubCommand(innerCommands, selection);
			},
			'Control+n': () => {
				selection = nextSubCommand(innerCommands, selection);
			},
			'Control+p': () => {
				selection = previousSubCommand(innerCommands, selection);
			}
		});
	});

	onDestroy(() => {
		unsubscribeKeyboardHandler?.();
	});

	export let userInput: string;

	$: innerCommands = [
		{
			title: 'Last 1 hour',
			description: 'Command',
			selected: false,
			action: {
				href: '/foo'
			},
			visible: 'last 1 hour'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Last 3 hours',
			description: 'Command',
			selected: false,
			action: {
				href: '/foo'
			},
			visible: 'last 3 hours'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Last 6 hours',
			description: 'Command',
			selected: false,
			action: {
				href: '/foo'
			},
			visible: 'last 6 hours'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Yesterday morning',
			description: 'Command',
			selected: false,
			action: {
				href: '/foo'
			},
			visible: 'yesterday morning'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Yesterday afternoon',
			description: 'Command',
			selected: false,
			action: {
				href: '/foo'
			},
			visible: 'yesterday afternoon'.includes(userInput?.toLowerCase())
		}
	] as Command[];

	let selection = 0;

	$: if (!innerCommands[selection]?.visible) {
		selection = firstVisibleSubCommand(innerCommands);
	}

	const firstVisibleSubCommand = (commands: Command[]): number => {
		const firstVisibleGroup = commands.findIndex((command) => command.visible);
		if (firstVisibleGroup === -1) {
			return 0;
		}
		return firstVisibleGroup;
	};

	const nextSubCommand = (commands: Command[], selection: number): number => {
		const nextVisibleCommandIndex = commands
			.slice(selection + 1)
			.findIndex((command) => command.visible);

		if (nextVisibleCommandIndex !== -1) {
			return selection + 1 + nextVisibleCommandIndex;
		}
		return 0;
	};

	const previousSubCommand = (commands: Command[], selection: number): number => {
		const previousVisibleCommandIndex = commands
			.slice(0, selection)
			.reverse()
			.findIndex((command) => command.visible);
		if (previousVisibleCommandIndex !== -1) {
			return selection - 1 - previousVisibleCommandIndex;
		}
		return commands.length - 1;
	};
</script>

<div class="mx-2 cursor-default select-none">
	<p class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300/80">
		Replay...
	</p>

	<ul class="">
		{#each innerCommands as command, commandIdx}
			{#if command.visible}
				{#if Action.isLink(command.action)}
					<a
						on:mouseover={() => (selection = commandIdx)}
						on:focus={() => (selection = commandIdx)}
						href={command.action.href}
						class="{selection === commandIdx
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
