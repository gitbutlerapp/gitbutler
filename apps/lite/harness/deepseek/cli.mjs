// oxlint-disable -- stdio CLI over the harness host bundle: plain JS whose payloads arrive untyped from the plugin sandbox.
/**
 * The stdio CLI over the harness host bundle.
 *
 * The DSH plugin host runs in a `node:vm` sandbox that cannot `import` a
 * module or load the native SDK, so it spawns this file (`node deepseek/cli.mjs`)
 * and speaks line-delimited JSON over stdin/stdout, exactly like the old MCP
 * bridge. Everything SDK-backed lives here: the endpoint table, the project
 * watchers, and `resolveProject`. The event emit the watcher needs is the
 * drain queue this file owns.
 *
 * Protocol (one JSON object per line, in both directions):
 *   -> { "id": 1, "method": "invoke", "endpoint": "...", "params": ... }
 *   -> { "id": 2, "method": "resolveProject", "candidates": ["..."] }
 *   -> { "id": 3, "method": "drain" }            // long-poll, 15s keep-alive
 *   -> { "id": 4, "method": "ping" }
 *   <- { "id": 1, "result": ... }                // or { "id": 1, "error": "..." }
 *   <- { "type": "event", "channel": "...", "payload": ... }   // unsolicited
 *
 * "drain" resolves with a batch of queued events as soon as one arrives, or
 * with `[]` after the keep-alive window — the client immediately re-arms, so
 * the panel refreshes only when a real watcher event lands, never on a tick.
 */
import { createButIpcHandler, resolveProject } from "../../dist/harness/node.js";
import path from "node:path";
import { createInterface } from "node:readline";
import process from "node:process";

const KEEP_ALIVE_MS = 15_000;

// ---------------------------------------------------------------------------
// Host-only handlers the endpoint table cannot derive (see HostOnlyKey in
// electron/src/endpoint-table.ts). Read-only where mutation makes no sense on
// a harness, inert where the real electron host would pop a dialog.
// ---------------------------------------------------------------------------

const defaultAiConfiguration = () => ({
	provider: "openai",
	openaiKeyOption: "butlerAPI",
	openaiModel: "gpt-4o",
	openaiHasApiKey: false,
	anthropicKeyOption: "butlerAPI",
	anthropicModel: "claude-sonnet-4-20250514",
	anthropicHasApiKey: false,
	ollamaEndpoint: "http://localhost:11434",
	ollamaModel: "",
	lmstudioEndpoint: "http://localhost:1234",
	lmstudioModel: "",
	isConfigured: false,
});

const hostOverrides = {
	getVersion: () => "gitbutler-harness",
	// The harness cannot open the user's browser; the panel degrades silently.
	openInWebBrowser: () => undefined,
	pickDirectory: () => null,
	clipboardWriteText: () => undefined,
	getAiConfiguration: () => defaultAiConfiguration(),
	updateAiConfiguration: () => defaultAiConfiguration(),
	resetAiConfiguration: () => defaultAiConfiguration(),
	// The renderer merges this over its own defaults (defaultSettings in
	// ui/src/settings.ts), so an empty versioned config is a valid read.
	readGUISettings: () => ({ version: 1 }),
	writeGUISettings: () => undefined,
	askpassSubmitPromptResponse: () => undefined,
};

// Imperative endpoints the endpoint table excludes; the plugin client answers
// showNativeMenu itself (browser popup), everything else is inert here.
const intercepts = {
	// pathJoin: variadic, so the client sends the whole argument array.
	// Node's own join, so separators and edge cases match electron main.
	pathJoin: (paths) => path.join(...paths),
	showNativeMenu: () => null,
	isFullScreen: () => false,
	streamAiResponse: () => "",
};

// ---------------------------------------------------------------------------
// The drain queue the watcher's emit feeds.
// ---------------------------------------------------------------------------

let queue = [];
const pendingDrains = [];

const emit = (channel, payload) => {
	queue.push({ channel, payload });
	const resolver = pendingDrains.shift();
	if (resolver) {
		const batch = queue;
		queue = [];
		resolver(batch);
	}
};

const drain = () =>
	new Promise((resolve) => {
		if (queue.length > 0) {
			const batch = queue;
			queue = [];
			resolve(batch);
			return;
		}
		let timer;
		// Settling has to unregister as well as resolve: a keep-alive that left
		// its resolver behind would be shifted by the next emit, which drains
		// the queue into an already-settled promise and loses those events.
		const settle = (batch) => {
			clearTimeout(timer);
			const waiting = pendingDrains.indexOf(settle);
			if (waiting !== -1) pendingDrains.splice(waiting, 1);
			resolve(batch);
		};
		timer = setTimeout(() => settle([]), KEEP_ALIVE_MS);
		pendingDrains.push(settle);
	});

const handle = createButIpcHandler({ emit, hostOverrides });

// ---------------------------------------------------------------------------
// Line-delimited JSON transport.
// ---------------------------------------------------------------------------

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

const respond = (id, result) => {
	// `undefined` is not JSON: a void endpoint would otherwise drop the result
	// key and the host would return `undefined` from its handler, which the DSH
	// RPC rejects. Null is the lossless way to say "no value".
	process.stdout.write(`${JSON.stringify({ id, result: result === undefined ? null : result })}\n`);
};

const respondError = (id, error) => {
	process.stdout.write(`${JSON.stringify({ id, error: String(error?.message ?? error) })}\n`);
};

const dispatch = async (message) => {
	const { id, method } = message;
	try {
		switch (method) {
			case "invoke": {
				const { endpoint, params } = message;
				const intercepted = intercepts[endpoint];
				try {
					respond(id, intercepted ? await intercepted(params) : await handle({ endpoint, params }));
				} catch (error) {
					// Name the endpoint so a failing call is identifiable from the
					// host-side error alone.
					respondError(id, new Error(`${endpoint}: ${error?.message ?? error}`));
				}
				return;
			}
			case "resolveProject":
				respond(id, await resolveProject(message.candidates ?? []));
				return;
			case "drain":
				respond(id, await drain());
				return;
			case "ping":
				respond(id, { pid: process.pid, platform: process.platform, node: process.version });
				return;
			default:
				respondError(id, new Error(`Unknown method: ${method}`));
		}
	} catch (error) {
		respondError(id, error);
	}
};

rl.on("line", (line) => {
	const trimmed = line.trim();
	if (trimmed === "") return;
	let message;
	try {
		message = JSON.parse(trimmed);
	} catch {
		// A malformed line is a protocol error, not a crash.
		process.stderr.write(`deepseek-cli: dropped malformed line: ${trimmed}\n`);
		return;
	}
	void dispatch(message);
});

process.on("uncaughtException", (error) => {
	process.stderr.write(`deepseek-cli: uncaught: ${error?.stack ?? error}\n`);
});
