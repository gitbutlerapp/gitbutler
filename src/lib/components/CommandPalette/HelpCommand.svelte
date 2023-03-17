<script lang="ts">
	import { Action, firstVisibleSubCommand, nextSubCommand, previousSubCommand } from './commands';
	import tinykeys from 'tinykeys';
	import { onDestroy, onMount } from 'svelte';
	import { open } from '@tauri-apps/api/shell';
	import { goto } from '$app/navigation';

	let selection = 0;

	$: selectedCommand = innerCommands[selection];

	const triggerCommand = () => {
		if (!innerCommands[selection].visible) {
			return;
		}
		if (Action.isLink(selectedCommand.action)) {
			console.log('triggerCommand');
			// toggleCommandPalette();
			// goto(selectedCommand.action.href);
			open(selectedCommand.action.href);
		}
	};

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
			},
			Enter: () => {
				triggerCommand();
			}
		});
	});

	onDestroy(() => {
		unsubscribeKeyboardHandler?.();
	});

	export let userInput: string;

	$: innerCommands = [
		{
			title: 'Open Documentation',
			description: 'External link',
			selected: false,
			action: {
				href: 'https://docs.gitbutler.com'
			},
			visible: 'documentation'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Join Discord Server',
			description: 'External link',
			selected: false,
			action: {
				href: 'https://discord.gg/MmFkmaJ42D'
			},
			visible: 'discord server'.includes(userInput?.toLowerCase())
		},
		{
			title: 'Email Support',
			description: 'External link',
			selected: false,
			action: {
				href: 'mailto:hello@gitbutler.com'
			},
			visible: 'discord server'.includes(userInput?.toLowerCase())
		}
	];
</script>

<div class="mx-2 cursor-default select-none">
	<p class="mx-2 cursor-default select-none py-2 text-sm font-semibold text-zinc-300/80">Help</p>

	<ul class="">
		{#each innerCommands as command, commandIdx}
			{#if command.visible}
				{#if Action.isLink(command.action)}
					<a
						target="_blank"
						rel="noreferrer"
						on:mouseover={() => (selection = commandIdx)}
						on:focus={() => (selection = commandIdx)}
						on:click={triggerCommand}
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
