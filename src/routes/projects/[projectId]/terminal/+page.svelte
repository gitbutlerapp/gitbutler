<script lang="ts">
	import { onMount } from 'svelte';
	import 'xterm/css/xterm.css';
	import * as xterm from 'xterm';
	import * as fit from 'xterm-addon-fit';
	import ResizeObserver from 'svelte-resize-observer';

	let terminalElement: HTMLElement;
	let terminalController: xterm.Terminal;
	let termFit: fit.FitAddon;
	let ptyWebSocket: WebSocket;
	const PTY_WS_ADDRESS = 'ws://127.0.0.1:7703';

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
		terminalController.onData((data) => termInterfaceHandleUserInputData(data));
		terminalController.focus();
	}

	const newTerminalSession = async () => {
		ptyWebSocket = new WebSocket(PTY_WS_ADDRESS);
		ptyWebSocket.binaryType = 'arraybuffer';
		ptyWebSocket.onmessage = writePtyIncomingToTermInterface;
		ptyWebSocket.onclose = (evt) => handlePtyWsClose(evt);
		ptyWebSocket.onerror = (evt) => handlePtyWsError(evt);
		ptyWebSocket.onopen = async (_evt) => initalizeXterm();
	};

	const writePtyIncomingToTermInterface = (evt) => {
		if (!(evt.data instanceof ArrayBuffer)) {
			alert('unknown data type ' + evt.data);
			return;
		}
		const dataString: string = arrayBufferToString(evt.data.slice(1));
		terminalController.write(dataString);
		return dataString;
	};

	const termInterfaceHandleUserInputData = (data: string) => {
		const encodedData = new TextEncoder().encode('\x00' + data);
		ptyWebSocket.send(encodedData);
	};

	const arrayBufferToString = (buf: ArrayBuffer) => {
		return String.fromCharCode.apply(null, new Uint8Array(buf));
	};

	const handlePtyWsClose = (_evt) => {
		terminalController.write('Terminal session terminated');
		terminalController.dispose();
		console.log('websocket closes from backend side');
	};

	const handlePtyWsError = (evt) => {
		if (typeof console.log == 'function') {
			console.log('ws error', evt);
		}
	};

	onMount(async () => {
		newTerminalSession();
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
