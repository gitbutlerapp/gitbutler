<script lang="ts">
	import type { ComponentType } from 'svelte';
	import { onMount } from 'svelte';
	import tinykeys, { type KeyBindingMap } from 'tinykeys';
	import CmdK from './CmdK.svelte';
	import Commit from './Commit.svelte';
	import Replay from './Replay.svelte';
	import { goto } from '$app/navigation';
	import type { Project } from '$lib/projects';
	import { readable, type Readable } from 'svelte/store';

	let dialog: ComponentType | undefined;
	let props: Record<string, unknown> = {};

	export let projects: Readable<Project[]>;
	export let project = readable<Project | undefined>(undefined);

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

	onMount(() => {
		const unsubscribeKeyboardHandler = hotkeys(
			window,
			{
				'Meta+k': () => {
					dialog === CmdK
						? (dialog = undefined)
						: ((dialog = CmdK), (props = { projects, project }));
				}
			},
			false // works even when an input is focused
		);
		const unsubscribeKeyboardHandlerDisabledOnInput = hotkeys(
			window,
			{
				c: () => {
					if ($project) {
						dialog === Commit ? (dialog = undefined) : ((dialog = Commit), (props = { project }));
					}
				},
				'Shift+c': () => {
					if ($project) {
						goto(`/projects/${$project.id}/commit`);
					}
				},
				r: () => {
					if ($project) {
						dialog === Replay ? (dialog = undefined) : ((dialog = Replay), (props = { project }));
					}
				}
			},
			true // disabled when an input is focused
		);
		return () => {
			unsubscribeKeyboardHandler();
			unsubscribeKeyboardHandlerDisabledOnInput();
		};
	});

	const onDialogClose = () => {
		dialog = undefined;
		props = {};
	};

	const onNewDialog = (e: CustomEvent) => {
		dialog = e.detail.component;
		props = e.detail.props;
	};
</script>

<svelte:component this={dialog} on:close={onDialogClose} on:newdialog={onNewDialog} {...props} />
