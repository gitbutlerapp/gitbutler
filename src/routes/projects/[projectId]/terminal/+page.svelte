<script lang="ts">
	import { onMount } from 'svelte';
	import 'xterm/css/xterm.css';
	import * as xterm from 'xterm';
	import * as fit from 'xterm-addon-fit';
	import ResizeObserver from 'svelte-resize-observer';

	let terminalElement: HTMLElement;
	let terminalController: xterm.Terminal;
	let termFit: fit.FitAddon;
	$: {
		if (terminalController) {
			// ...
		}
	}
	function initalizeXterm() {
		terminalController = new xterm.Terminal();
		termFit = new fit.FitAddon();
		terminalController.loadAddon(termFit);
		terminalController.open(terminalElement);
		termFit.fit();
		terminalController.write('I am a terminal!');
		terminalController.onData((e) => {
			console.log(e);
		});
	}
	onMount(async () => {
		initalizeXterm();
	});
	function handleTermResize() {
		if (termFit) {
			termFit.fit();
		}
	}
</script>

<!-- Actual terminal -->
<div id="terminal" class="w-full h-full" bind:this={terminalElement} />

<!-- Resize observer -->
<div class="absolute top-0 bottom-0 left-0 right-0">
	<ResizeObserver on:resize={handleTermResize} />
</div>
