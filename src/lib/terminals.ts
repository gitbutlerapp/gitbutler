import { writable } from 'svelte/store';
import type { Terminal } from 'xterm';
import type { FitAddon } from 'xterm-addon-fit';

import * as xterm from 'xterm';
import * as fit from 'xterm-addon-fit';
import { CanvasAddon } from 'xterm-addon-canvas';
import { Unicode11Addon } from 'xterm-addon-unicode11';

const PTY_WS_ADDRESS = 'ws://127.0.0.1:7703';

export type TerminalSession = {
  projectId: string;
  element: HTMLElement;
	controller: Terminal;
	fit: FitAddon;
  pty: WebSocket;
};

export const newTerminalSession = async (session: TerminalSession) => {
  session.pty = new WebSocket(PTY_WS_ADDRESS);
  session.pty.binaryType = 'arraybuffer';
  session.pty.onmessage = (evt) => writePtyIncomingToTermInterface(evt, session);
  session.pty.onclose = (evt) => handlePtyWsClose(evt, session);
  session.pty.onerror = (evt) => handlePtyWsError(evt, session);
  session.pty.onopen = async (_evt) => initalizeXterm(session);
};

export function focus(session: TerminalSession) {
  console.log('focus');
  session.controller.focus();
}

function initalizeXterm(session: TerminalSession) {
  console.log('initalizeXterm')
  session.controller = new xterm.Terminal({
    cursorBlink: false,
    cursorStyle: 'block',
    fontSize: 13,
    rows: 24,
    cols: 80,
    allowProposedApi: true
  });

  session.controller.loadAddon(new Unicode11Addon());
  session.controller.unicode.activeVersion = '11';

  session.fit = new fit.FitAddon();
  session.controller.loadAddon(session.fit);
  session.controller.loadAddon(new CanvasAddon());
  session.controller.open(session.element);
  session.fit.fit();
  session.controller.onData((data) => termInterfaceHandleUserInputData(data, session));
  focus(session);
}

const writePtyIncomingToTermInterface = (evt: MessageEvent, session: TerminalSession) => {
  if (!(evt.data instanceof ArrayBuffer)) {
    alert('unknown data type ' + evt.data);
    return;
  }
  console.log('terminal input', evt.data);
  const dataString: string = arrayBufferToString(evt.data.slice(1));
  console.log('terminal input string', dataString);
  session.controller.write(dataString);
  return dataString;
};

const termInterfaceHandleUserInputData = (data: string, session: TerminalSession) => {
  console.log('user input', data);
  const encodedData = new TextEncoder().encode('\x00' + data);
  session.pty.send(encodedData);
};

const arrayBufferToString = (buf: ArrayBuffer) => {
  return String.fromCharCode.apply(null, new Uint8Array(buf));
};

const handlePtyWsClose = (evt: Event, session: TerminalSession) => {
  session.controller.write('Terminal session terminated');
  session.controller.dispose();
  console.log('websocket closes from backend side');
};

const handlePtyWsError = (evt: Event, session: TerminalSession) => {
  if (typeof console.log == 'function') {
    console.log('ws error', evt);
  }
};

export const terminals = writable([] as TerminalSession[]);