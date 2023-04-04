<script lang="ts">
	import 'xterm/css/xterm.css';
	import ResizeObserver from 'svelte-resize-observer';
	import * as terminals from '$lib/terminals';
	import { onMount } from 'svelte';

	export let session: terminals.TerminalSession;

	onMount(() => {
		terminals.newTerminalSession(session);
	});

	function handleTermResize() {
		if (session.fit) {
			session.fit.fit();
		}
	}

	export const runCommand = (command: string): void => {
		command = command + '\r';
		console.log('command input', command);
		const encodedData = new TextEncoder().encode('\x00' + command);
		session.pty.send(encodedData);
	};
</script>

<!-- Actual terminal -->
<div class="flex flex-row w-full h-full">
	<div
		id="terminal"
		class="w-full h-full"
		bind:this={session.element}
		on:click={focus}
		on:keydown={focus}
	/>
	<ResizeObserver on:resize={handleTermResize} />
</div>
