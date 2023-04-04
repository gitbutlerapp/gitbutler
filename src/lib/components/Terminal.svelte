<script lang="ts">
	import 'xterm/css/xterm.css';
	import ResizeObserver from 'svelte-resize-observer';
	import * as terminals from '$lib/terminals';
	import { onMount } from 'svelte';

	export let session: terminals.TerminalSession;

	onMount(() => {
		session.controller?.open(session.element);
	});

	function handleTermResize() {
		terminals.fitSession(session);
	}

	export const runCommand = (command: string): void => {
		command = command + '\r';
		console.log('command input', command);
		const encodedData = new TextEncoder().encode('\x00' + command);
		session.pty.send(encodedData);
	};
</script>

<!-- Actual terminal -->
<div class="flex h-full w-full flex-row">
	<div
		id="terminal"
		class="h-full w-full"
		bind:this={session.element}
		on:click={focus}
		on:keydown={focus}
	/>
	<ResizeObserver on:resize={handleTermResize} />
</div>
