<script lang="ts">
	import { onDestroy, onMount, afterUpdate } from 'svelte';
	import { currentProject } from '$lib/current_project';
	import { IconCircleCancel } from '$lib/components/icons';
	import tinykeys from 'tinykeys';

	$: scopeToProject = $currentProject ? true : false;

	let showingCommandPalette = false;
	let dialog: HTMLDialogElement;
	let userInput: string;

	const toggleCommandPalette = () => {
		if (dialog && dialog.open) {
			dialog.close();
			showingCommandPalette = false;
		} else {
			dialog.showModal();
			showingCommandPalette = true;
		}
	};

	let unsubscribeKeyboardHandler: () => void;

	onMount(() => {
		toggleCommandPalette();
		unsubscribeKeyboardHandler = tinykeys(window, {
			'Meta+k': () => {
				toggleCommandPalette();
			},
			Backspace: () => {
				if (!userInput) {
					scopeToProject = false;
				}
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
			<div class="ml-4 flex flex-grow items-center">
				<!-- Project scope -->
				{#if scopeToProject}
					<div class="flex items-center">
						<span class="font-semibold text-zinc-300">{$currentProject?.title}</span>
						<span class="ml-1 text-lg">/</span>
					</div>
				{/if}
				<!-- Search input -->
				<div class="mx-1 flex-grow">
					<input
						class="w-full bg-transparent text-zinc-300 focus:outline-none"
						bind:value={userInput}
						type="text"
						placeholder={scopeToProject
							? 'Search for commands, files and code changes...'
							: 'Search for projects'}
					/>
				</div>
				<div class="mr-4 text-red-50">
					<IconCircleCancel class="fill-zinc-400" />
				</div>
			</div>
		</div>
	</div>
</dialog>
