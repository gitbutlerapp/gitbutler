<script lang="ts">
	import type { ComponentType } from 'svelte';

	import { onDestroy, onMount } from 'svelte';
	import tinykeys, { type KeyBindingMap } from 'tinykeys';
	import CmdK from './CmdK.svelte';
	import Commit from './Commit.svelte';
	import Replay from './Replay.svelte';
	import Branch from './Branch.svelte';
	import { currentProject } from '$lib/current_project';

	let dialog: ComponentType | undefined;

	function isEventTargetInputOrTextArea(target: any) {
		if (target === null) return false;

		const targetElementName = target.tagName.toLowerCase();
		return ['input', 'textarea'].includes(targetElementName);
	}

	function hotkeys(target: Window | HTMLElement, bindings: KeyBindingMap, disableOnInputs = true) {
		const wrappedBindings = disableOnInputs
			? Object.fromEntries(
					Object.entries(bindings).map(([key, handler]) => [
						key,
						(event: KeyboardEvent) => {
							if (!isEventTargetInputOrTextArea(event.target)) {
								handler(event);
							}
						}
					])
			  )
			: bindings;
		return tinykeys(target, wrappedBindings);
	}

	let unsubscribeKeyboardHandler: () => void;
	let unsubscribeKeyboardHandlerDisabledOnInput: () => void;
	onMount(() => {
		unsubscribeKeyboardHandler = hotkeys(
			window,
			{
				'Meta+k': () => {
					dialog === CmdK ? (dialog = undefined) : (dialog = CmdK);
				}
			},
			false // works even when an input is focused
		);
		unsubscribeKeyboardHandlerDisabledOnInput = hotkeys(
			window,
			{
				c: () => {
					if ($currentProject) {
						dialog === Commit ? (dialog = undefined) : (dialog = Commit);
					}
				},
				r: () => {
					if ($currentProject) {
						dialog === Replay ? (dialog = undefined) : (dialog = Replay);
					}
				},
				b: () => {
					if ($currentProject) {
						dialog === Branch ? (dialog = undefined) : (dialog = Branch);
					}
				}
			},
			true // disabled when an input is focused
		);
	});

	onDestroy(() => {
		unsubscribeKeyboardHandler?.();
		unsubscribeKeyboardHandlerDisabledOnInput?.();
	});

	const onDialogClose = () => {
		dialog = undefined;
	};

	const onNewDialog = (e: CustomEvent) => {
		dialog = e.detail;
	};
</script>

{#if dialog}
	<svelte:component this={dialog} on:close={onDialogClose} on:newdialog={onNewDialog} />
{/if}
