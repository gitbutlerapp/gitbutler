/**
 * IRC Session Bridge
 *
 * Bridges Claude codegen sessions to IRC, enabling:
 * - Broadcasting session state to IRC channels
 * - Receiving commands from remote IRC clients
 * - Session discovery via list requests
 *
 * Uses RTKQ (IrcApiService) for IRC mutations and IBackend.listen for
 * persistent message subscriptions.
 *
 * Note: This service must be instantiated within a Svelte component context
 * for the reactive effects to work properly.
 */

import { type ClaudeCodeService } from "$lib/codegen/claude";
import { IRC_CONNECTION_ID, type IrcApiService } from "$lib/irc/ircApiService";
import {
	Messages,
	parse,
	parseTextCommand,
	serialize,
	sessionChannel,
	type IrcProtocolMessage,
	type PlainTextCommand,
} from "$lib/irc/protocol";
import { type SettingsService } from "$lib/settings/appSettings";
import { InjectionToken } from "@gitbutler/core/context";
import { persistWithExpiration, type Persisted } from "@gitbutler/shared/persisted";
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
import { SvelteMap } from "svelte/reactivity";
import { get } from "svelte/store";
import type { IBackend } from "$lib/backend/backend";
import type {
	ClaudeMessage,
	ClaudeStatus,
	PermissionDecision,
	SystemMessage,
} from "$lib/codegen/types";
import type { StoredMessage } from "$lib/irc/ircEndpoints";
import type { Reactive } from "@gitbutler/shared/storeUtils";

export const IRC_SESSION_BRIDGE = new InjectionToken<IrcSessionBridge>("IrcSessionBridge");

/**
 * Tracks an active session being bridged to IRC.
 */
type BridgedSession = {
	projectId: string;
	stackId: string;
	branchName?: string;
	channel: string;
	/** Last known status */
	status: ClaudeStatus;
	/** Whether the bridge is fully set up (IRC + Claude listeners active) */
	connected: boolean;
	/** Unsubscribe function for the IRC message listener */
	unsubscribeIrc?: () => void;
	/** Unsubscribe function for the Claude message listener */
	unsubscribeClaude?: () => void;
	/** Track pending permission requests by short ID */
	pendingPermissions: Map<string, string>; // shortId -> fullId
	/** Track pending questions by short ID */
	pendingQuestions: Map<string, string>; // shortId -> toolUseId
};

export class IrcSessionBridge {
	/** Active bridged sessions by stack ID */
	private sessions = new SvelteMap<string, BridgedSession>();
	/** Cached bot nick */
	private myNick: string | undefined;
	/** Per-stack persisted stores for manual bridging (keyed by stack ID, 7-day TTL) */
	private manualBridgeStores = new Map<string, Persisted<boolean>>();

	constructor(
		private backend: IBackend,
		private ircApiService: IrcApiService,
		private claudeCodeService: ClaudeCodeService,
		private settingsService: SettingsService,
	) {}

	/** Get the user's IRC nick from settings (the account owner who may send commands). */
	private getOwnerNick(): string | undefined {
		return get(this.settingsService.appSettings)?.irc.connection.nickname ?? undefined;
	}

	/**
	 * Start bridging a session to IRC.
	 *
	 * Registers the session. The caller is responsible for driving
	 * connect/disconnect via `setBotReady`.
	 * Call `stopBridging` to tear everything down.
	 */
	startBridging(params: { projectId: string; stackId: string; branchName: string }): void {
		const { projectId, stackId, branchName } = params;
		const ownerNick = this.getOwnerNick() || "unknown";
		const channel = sessionChannel(ownerNick, branchName);

		if (this.sessions.has(stackId)) return;

		const session: BridgedSession = {
			projectId,
			stackId,
			branchName,
			channel,
			status: "enabled",
			connected: false,
			pendingPermissions: new Map(),
			pendingQuestions: new Map(),
		};

		this.sessions.set(stackId, session);
	}

	/**
	 * Notify the bridge that the bot connection readiness changed.
	 * Connects or disconnects the bridge for the given session accordingly.
	 */
	setBotReady(stackId: string, ready: boolean): void {
		const session = this.sessions.get(stackId);
		if (!session) return;

		if (ready && !session.connected) {
			this.connectBridge(session);
		} else if (!ready && session.connected) {
			this.disconnectBridge(session);
		}
	}

	/**
	 * Activate the bridge for a session: join channel, subscribe to messages.
	 */
	private async connectBridge(session: BridgedSession): Promise<void> {
		if (session.connected) return;
		session.connected = true;

		try {
			this.myNick = await this.ircApiService.fetchNick();
		} catch {
			// Non-critical
		}

		try {
			await this.ircApiService.joinChannel({ channel: session.channel });
		} catch (e) {
			console.warn("[IrcSessionBridge] Failed to join channel:", e);
		}

		session.unsubscribeClaude = this.claudeCodeService.onMessage(
			session.projectId,
			session.stackId,
			(message) => {
				this.publishClaudeMessage(session, message);
			},
		);

		session.unsubscribeIrc = wrapUnsubscribe(
			this.backend.listen<StoredMessage>(`irc:${IRC_CONNECTION_ID}:message`, (event) => {
				const msg = event.payload;
				if (msg.target !== session.channel) return;

				// Only accept messages from the session owner (same nick via bouncer/multi-client)
				const ownerNick = this.getOwnerNick();
				if (ownerNick && msg.sender !== ownerNick) return;

				// Skip protocol messages (bridge's own publish() calls carry +data).
				if (msg.data) return;

				this.handleNewIrcMessage(session, {
					channel: msg.target,
					from: msg.sender,
					text: msg.content,
					data: msg.data ?? undefined,
				});
			}),
		);
	}

	/**
	 * Deactivate the bridge for a session: unsubscribe from messages, part channel.
	 */
	private disconnectBridge(session: BridgedSession): void {
		session.unsubscribeClaude?.();
		session.unsubscribeClaude = undefined;
		session.unsubscribeIrc?.();
		session.unsubscribeIrc = undefined;
		session.connected = false;
	}

	/**
	 * Stop bridging a session.
	 */
	stopBridging(stackId: string, exitCode: number = 0): void {
		const session = this.sessions.get(stackId);

		if (!session) return;

		// Tear down the active bridge if connected
		if (session.connected) {
			this.disconnectBridge(session);

			// Announce session end and leave channel
			this.publish(session.channel, Messages.sessionEnd({ code: exitCode }));
			this.ircApiService.partChannel({ channel: session.channel }).catch(() => {});
		}

		// Cleanup - create new map to trigger reactivity
		this.sessions.delete(stackId);
	}

	/**
	 * Publish a protocol message to an IRC channel.
	 */
	private publish(channel: string, message: IrcProtocolMessage): void {
		const { text, data, truncated } = serialize(message);
		this.sendToChannel(channel, text, data);

		if (truncated) {
			console.warn(`[IrcSessionBridge] Message truncated for ${channel}:`, message.type);
		}
	}

	/**
	 * Send a message to an IRC channel, optionally with a data payload.
	 */
	private sendToChannel(channel: string, message: string, data?: unknown): void {
		// Never send empty/whitespace-only text messages
		if (!data && !message.trim()) return;

		if (data) {
			const encoded = typeof data === "string" ? data : JSON.stringify(data);
			// Data messages are always filtered by the `msg.data` check in the
			// listener, so no need to track them here.
			this.ircApiService
				.sendMessageWithData({
					target: channel,
					message,
					data: encoded,
				})
				.catch((e) => {
					console.warn("[IrcSessionBridge] Failed to send message with data:", e);
				});
		} else {
			this.ircApiService
				.sendMessage({
					target: channel,
					message,
				})
				.catch((e) => {
					console.warn("[IrcSessionBridge] Failed to send message:", e);
				});
		}
	}

	/**
	 * Handle a new incoming IRC message.
	 * Only accepts commands from the configured user nick (session owner).
	 */
	private handleNewIrcMessage(
		session: BridgedSession,
		msg: { channel: string; from: string; text: string; data?: string },
	): void {
		// Try to parse as protocol message first (from +data tag)
		if (msg.data) {
			try {
				const parsed = parse(msg.data);
				if (parsed) {
					this.handleProtocolMessage(session, parsed, msg.from);
					return;
				}
			} catch {
				// Failed to decode/parse, fall through to text command parsing
			}
		}

		// Only process explicit bang commands (!prompt, !approve, etc.)
		// to prevent Claude's own output from being re-interpreted as prompts.
		// parseTextCommand treats bare text as "prompt" type, which would cause
		// infinite loops when bridge-sent messages leak through echo filtering.
		if (!msg.text.trimStart().startsWith("!")) return;

		const command = parseTextCommand(msg.text);
		if (command.type !== "unknown") {
			this.handleTextCommand(session, command, msg.from);
		}
	}

	/**
	 * Handle a parsed protocol message from IRC.
	 */
	private handleProtocolMessage(
		session: BridgedSession,
		message: IrcProtocolMessage,
		sender: string,
	): void {
		switch (message.type) {
			case "prompt":
				this.executePrompt(session, message.payload.message, sender).catch((e) =>
					console.warn("[IrcSessionBridge] executePrompt failed:", e),
				);
				break;

			case "permission-decision":
				this.executePermissionDecision(
					session,
					message.payload.requestId,
					message.payload.decision,
					sender,
				).catch((e) => console.warn("[IrcSessionBridge] executePermissionDecision failed:", e));
				break;

			case "answer":
				this.executeAnswer(
					session,
					message.payload.toolUseId,
					message.payload.answers,
					sender,
				).catch((e) => console.warn("[IrcSessionBridge] executeAnswer failed:", e));
				break;

			case "abort":
				this.executeAbort(session, sender).catch((e) =>
					console.warn("[IrcSessionBridge] executeAbort failed:", e),
				);
				break;

			case "session-list-request":
				this.handleSessionListRequest(session.channel, message.payload.projectId);
				break;
		}
	}

	/**
	 * Handle a plain text command from IRC.
	 */
	private handleTextCommand(
		session: BridgedSession,
		command: PlainTextCommand,
		sender: string,
	): void {
		switch (command.type) {
			case "prompt":
				this.executePrompt(session, command.message, sender).catch((e) =>
					console.warn("[IrcSessionBridge] executePrompt failed:", e),
				);
				break;

			case "approve":
				this.executePermissionDecision(
					session,
					this.resolveShortId(session.pendingPermissions, command.requestId),
					"allowOnce",
					sender,
				).catch((e) => console.warn("[IrcSessionBridge] executePermissionDecision failed:", e));
				break;

			case "deny":
				this.executePermissionDecision(
					session,
					this.resolveShortId(session.pendingPermissions, command.requestId),
					"denyOnce",
					sender,
				).catch((e) => console.warn("[IrcSessionBridge] executePermissionDecision failed:", e));
				break;

			case "abort":
				this.executeAbort(session, sender).catch((e) =>
					console.warn("[IrcSessionBridge] executeAbort failed:", e),
				);
				break;

			case "answer":
				this.executeAnswer(
					session,
					this.resolveLatestQuestion(session),
					command.answers,
					sender,
				).catch((e) => console.warn("[IrcSessionBridge] executeAnswer failed:", e));
				break;

			case "status":
				this.publishStatus(session);
				break;

			case "sessions":
				this.handleSessionListRequest(session.channel, command.projectId);
				break;
		}
	}

	/**
	 * Resolve a short ID to a full ID.
	 */
	private resolveShortId(map: Map<string, string>, shortOrFull: string): string {
		if (shortOrFull === "latest") {
			// Return the most recent one
			const entries = Array.from(map.entries());
			return entries.length > 0 ? entries[entries.length - 1]![1] : shortOrFull;
		}

		// Check if it's a short ID
		const fullId = map.get(shortOrFull);
		if (fullId) return fullId;

		// Assume it's already a full ID
		return shortOrFull;
	}

	/**
	 * Resolve the latest pending question.
	 */
	private resolveLatestQuestion(session: BridgedSession): string {
		const entries = Array.from(session.pendingQuestions.entries());
		return entries.length > 0 ? entries[entries.length - 1]![1] : "unknown";
	}

	// =========================================================================
	// Command Execution
	// =========================================================================

	private async executePrompt(
		session: BridgedSession,
		message: string,
		_sender: string,
	): Promise<void> {
		// Send to Claude - the user message will be broadcast via publishUserMessage
		// when it comes back through the onMessage listener
		await this.claudeCodeService.sendMessageMutate({
			projectId: session.projectId,
			stackId: session.stackId,
			message,
			thinkingLevel: "normal",
			model: "sonnet",
			permissionMode: "default",
			disabledMcpServers: [],
			addDirs: [],
		});
	}

	private async executePermissionDecision(
		session: BridgedSession,
		requestId: string,
		decision: PermissionDecision,
		sender: string,
	): Promise<void> {
		// Announce the decision
		this.publish(
			session.channel,
			Messages.permissionDecision({
				requestId,
				decision,
				decidedBy: sender,
			}),
		);

		// Execute the decision
		await this.claudeCodeService.updatePermissionRequest({
			projectId: session.projectId,
			requestId,
			decision,
			useWildcard: false,
		});

		// Remove from pending
		for (const [shortId, fullId] of session.pendingPermissions) {
			if (fullId === requestId) {
				session.pendingPermissions.delete(shortId);
				break;
			}
		}
	}

	private async executeAnswer(
		session: BridgedSession,
		toolUseId: string,
		answers: Record<string, string>,
		sender: string,
	): Promise<void> {
		// Announce the answer
		this.publish(
			session.channel,
			Messages.answer({
				toolUseId,
				answers,
				answeredBy: sender,
			}),
		);

		// Execute the answer
		await this.claudeCodeService.answerAskUserQuestion({
			projectId: session.projectId,
			stackId: session.stackId,
			answers,
		});

		// Remove from pending
		for (const [shortId, fullId] of session.pendingQuestions) {
			if (fullId === toolUseId) {
				session.pendingQuestions.delete(shortId);
				break;
			}
		}
	}

	private async executeAbort(session: BridgedSession, sender: string): Promise<void> {
		// Announce the abort
		this.publish(
			session.channel,
			Messages.abort({
				requestedBy: sender,
			}),
		);

		// Execute the abort
		await this.claudeCodeService.cancelSession({
			projectId: session.projectId,
			stackId: session.stackId,
		});
	}

	// =========================================================================
	// Session Updates (Outbound)
	// =========================================================================

	/**
	 * Publish a single Claude message to IRC.
	 * Called when a new message is received via the backend listener.
	 */
	private publishClaudeMessage(session: BridgedSession, message: ClaudeMessage): void {
		const { payload } = message;

		// Handle user messages - broadcast to IRC so remote users see prompts
		if (payload.source === "user") {
			this.publishUserMessage(session, payload);
			return;
		}

		// Handle claude messages (assistant responses)
		if (payload.source === "claude") {
			this.publishClaudeOutput(session, payload.data);
			return;
		}

		// Handle system messages (exit, abort, etc.)
		if (payload.source === "system") {
			const text = systemMessageText(payload);
			if (text) this.sendToChannel(session.channel, text);
			return;
		}

		// Handle gitButler updates (commit created, etc.)
		if (payload.source === "gitButler") {
			if (payload.type === "commitCreated") {
				const branch = payload.branchName ?? "unknown";
				const count = payload.commitIds.length;
				this.sendToChannel(
					session.channel,
					`[commit] ${count} commit${count !== 1 ? "s" : ""} created on ${branch}`,
				);
			}
		}
	}

	/**
	 * Publish Claude output (assistant messages, results, etc.) to IRC.
	 */
	private publishClaudeOutput(
		session: BridgedSession,
		claudeData: import("$lib/codegen/types").ClaudeCodeMessage,
	): void {
		// Only publish assistant messages for now
		if (claudeData.type !== "assistant") {
			return;
		}

		// claudeData.message is the Anthropic SDK Message type
		const content = claudeData.message.content;

		// Extract text content from the message
		const textContent = content
			.filter((c) => c.type === "text")
			.map((c) => (c as { type: "text"; text: string }).text)
			.join("\n");

		// Extract tool use information
		const toolUses = content
			.filter((c) => c.type === "tool_use")
			.map((c) => c as { type: "tool_use"; id: string; name: string; input: unknown });

		// Check for AskUserQuestion tool
		const askQuestion = toolUses.find((t) => t.name === "AskUserQuestion");
		if (askQuestion && askQuestion.input && typeof askQuestion.input === "object") {
			const input = askQuestion.input as { questions?: unknown[] };
			if (input.questions && Array.isArray(input.questions)) {
				const shortId = askQuestion.id.slice(0, 8);
				session.pendingQuestions.set(shortId, askQuestion.id);

				this.publish(
					session.channel,
					Messages.question({
						toolUseId: askQuestion.id,
						questions: input.questions as any,
					}),
				);
				return;
			}
		}

		// Publish text content if present (skip whitespace-only)
		if (textContent.trim()) {
			this.publishTextResponse(session.channel, textContent);
		}

		// Publish tool call summaries (but not the empty "(no text content)" placeholder)
		if (toolUses.length > 0) {
			const toolSummary = toolUses
				.filter((tc) => tc.name !== "AskUserQuestion")
				.map((tc) => `${tc.name}(${summarizeInput(tc.input)})`)
				.join(", ");
			if (toolSummary) {
				this.sendToChannel(session.channel, `[Tools] ${toolSummary}`);
			}
		}
	}

	/**
	 * Publish text response directly to IRC.
	 *
	 * The backend handles multiline messages via IRCv3 draft/multiline BATCH
	 * when the server supports it, falling back to individual PRIVMSGs per line.
	 */
	private publishTextResponse(channel: string, text: string): void {
		this.sendToChannel(channel, text);
	}

	/**
	 * Publish user messages (prompts) to IRC.
	 * This broadcasts what the user typed so remote IRC users can follow along.
	 */
	private publishUserMessage(
		session: BridgedSession,
		payload: import("$lib/codegen/types").MessagePayload & { source: "user" },
	): void {
		const message = payload.message?.trim();
		if (!message) return;

		// Send the user message as plain text, prefixed to indicate it's a prompt
		this.publishTextResponse(session.channel, `> ${message}`);
	}

	/**
	 * Publish current session status.
	 */
	private publishStatus(session: BridgedSession): void {
		this.publish(
			session.channel,
			Messages.sessionStatus({
				status: session.status,
				todos: [],
			}),
		);
	}

	// =========================================================================
	// Session Discovery
	// =========================================================================

	/**
	 * Handle a session list request.
	 */
	private handleSessionListRequest(responseChannel: string, filterProjectId?: string): void {
		const sessions = Array.from(this.sessions.values())
			.filter((s) => !filterProjectId || s.projectId === filterProjectId)
			.map((s) => ({
				projectId: s.projectId,
				stackId: s.stackId,
				branchName: s.branchName,
				status: s.status,
				channel: s.channel,
			}));

		this.publish(
			responseChannel,
			Messages.sessionListResponse({
				sessions,
			}),
		);
	}

	/**
	 * Clean up all bridged sessions.
	 */
	destroy(): void {
		for (const session of this.sessions.values()) {
			this.disconnectBridge(session);
			this.ircApiService.partChannel({ channel: session.channel }).catch(() => {});
		}
		this.sessions.clear();
	}

	/**
	 * Get the list of currently bridged sessions.
	 */
	getBridgedSessions(): BridgedSession[] {
		return Array.from(this.sessions.values());
	}

	/**
	 * Check if a session is being bridged.
	 */
	isBridging(stackId?: string): Reactive<boolean> {
		return reactive(() => (stackId ? this.sessions.has(stackId) : false));
	}

	/**
	 * Get or create the persisted store for a stack's manual bridge state.
	 */
	private getManualBridgeStore(stackId: string): Persisted<boolean> {
		let store = this.manualBridgeStores.get(stackId);
		if (!store) {
			store = persistWithExpiration<boolean>(false, `irc:manualBridge:${stackId}`, 60 * 24 * 7);
			this.manualBridgeStores.set(stackId, store);
		}
		return store;
	}

	/**
	 * Enable or disable manual bridging for a stack (persisted across reloads).
	 */
	setManualBridge(stackId: string, enabled: boolean): void {
		this.getManualBridgeStore(stackId).set(enabled);
	}

	/**
	 * Check if a stack has been manually enabled for bridging.
	 */
	isManuallyBridged(stackId?: string): Reactive<boolean> {
		if (!stackId) return reactive(() => false);
		const store = this.getManualBridgeStore(stackId);
		return reactive(() => get(store));
	}
}

// ============================================================================
// Helpers
// ============================================================================

/** Wrap an async listener result into a synchronous unsubscribe function. */
function wrapUnsubscribe(listenResult: Promise<() => void> | (() => void)): () => void {
	return () => {
		Promise.resolve(listenResult).then((fn) => {
			if (typeof fn === "function") fn();
		});
	};
}

function summarizeInput(input: unknown): string {
	if (typeof input === "string") {
		return input.length > 100 ? input.slice(0, 100) + "..." : input;
	}

	if (typeof input === "object" && input !== null) {
		const obj = input as Record<string, unknown>;

		// Common tool input patterns
		if ("file_path" in obj) return `file: ${obj.file_path}`;
		if ("path" in obj) return `path: ${obj.path}`;
		if ("command" in obj) return `cmd: ${String(obj.command).slice(0, 50)}`;
		if ("pattern" in obj) return `pattern: ${obj.pattern}`;
		if ("query" in obj) return `query: ${obj.query}`;
		if ("url" in obj) return `url: ${obj.url}`;

		// Fallback: stringify and truncate
		const str = JSON.stringify(obj);
		return str.length > 100 ? str.slice(0, 100) + "..." : str;
	}

	return String(input);
}

function systemMessageText(msg: { source: "system" } & SystemMessage): string | undefined {
	switch (msg.type) {
		case "claudeExit":
			return msg.code === 0
				? "[session] ended"
				: `[session] exited with code ${msg.code}: ${msg.message}`;
		case "userAbort":
			return "[session] aborted by user";
		case "unhandledException":
			return `[error] ${msg.message}`;
		case "rateLimited":
			return `[rate-limited] retrying in ${Math.ceil(msg.retryAfterMs / 1000)}s`;
		case "compactStart":
		case "compactFinished":
			return undefined;
	}
}
