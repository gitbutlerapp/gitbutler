<script lang="ts">
	import { onMount } from 'svelte';
	import 'xterm/css/xterm.css';
	import * as xterm from 'xterm';
	import * as fit from 'xterm-addon-fit';
	import { CanvasAddon } from 'xterm-addon-canvas';
	import { WebglAddon } from 'xterm-addon-webgl';
	import { LigaturesAddon } from 'xterm-addon-ligatures';
	import { Unicode11Addon } from 'xterm-addon-unicode11';
	import ResizeObserver from 'svelte-resize-observer';
	import { shortPath } from '$lib/paths';

	let terminalElement: HTMLElement;
	let terminalController: xterm.Terminal;
	let termFit: fit.FitAddon;
	let ptyWebSocket: WebSocket;

	const PTY_WS_ADDRESS = 'ws://127.0.0.1:7703';

	const webglIsSupported = () => {
		// looks like xterm-addon-webgl is not working with webgl2
		var canvas = document.createElement('canvas');
		var gl = canvas.getContext('webgl2');
		if (gl && gl instanceof WebGL2RenderingContext) {
			return true;
		} else {
			return false;
		}
	};

	function initalizeXterm() {
		terminalController = new xterm.Terminal({
			cursorBlink: false,
			cursorStyle: 'block',
			fontSize: 13,
			rows: 24,
			cols: 80,
			allowProposedApi: true
		});

		terminalController.loadAddon(new Unicode11Addon());
		terminalController.unicode.activeVersion = '11';

		termFit = new fit.FitAddon();
		terminalController.loadAddon(termFit);
		/*
		if (webglIsSupported()) {
			terminalController.loadAddon(new WebglAddon());
		} else {
		}
    */
		terminalController.loadAddon(new CanvasAddon());
		terminalController.open(terminalElement);
		terminalController.loadAddon(new LigaturesAddon());
		termFit.fit();
		terminalController.onData((data) => termInterfaceHandleUserInputData(data));
		focus();
	}

	function focus() {
		console.log('focus');
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
		//console.log('terminal input', evt.data);
		const dataString: string = arrayBufferToString(evt.data.slice(1));
		//console.log('terminal input string', dataString);
		terminalController.write(dataString);
		return dataString;
	};

	const termInterfaceHandleUserInputData = (data: string) => {
		//console.log('user input', data);
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

	function runCommand(command: string) {
		command = command + '\r';
		console.log('command input', command);
		const encodedData = new TextEncoder().encode('\x00' + command);
		ptyWebSocket.send(encodedData);
	}
</script>

<!-- Actual terminal -->
<div class="flex flex-row w-full h-full">
	<div
		id="terminal"
		class="w-full h-full"
		bind:this={terminalElement}
		on:click={focus}
		on:keydown={focus}
	/>
	<ResizeObserver on:resize={handleTermResize} />
</div>
