<script lang="ts">
	import { IRC_API_SERVICE } from "$lib/irc/ircApiService";
	import { inject } from "@gitbutler/core/context";
	import { Button } from "@gitbutler/ui";
	import { onDestroy } from "svelte";
	import type { StoredMessage } from "$lib/irc/ircApi";

	type Props = {
		type: "group" | "private" | "server";
		replyTo?: StoredMessage;
		onCancelReply?: () => void;
		onSent?: () => void;
	} & ({ type: "group"; channel: string } | { type: "private"; nick: string } | { type: "server" });

	const args: Props = $props();

	const ircApiService = inject(IRC_API_SERVICE);

	let input = $state("");
	let inputEl: HTMLTextAreaElement | undefined = $state();

	// ── Typing indicator ────────────────────────────────────────────────
	const TYPING_PAUSE_MS = 3000;
	let typingActive = false;
	let typingTimer: ReturnType<typeof setTimeout> | undefined;

	function getTarget(): string | undefined {
		if (args.type === "group") return args.channel;
		if (args.type === "private") return args.nick;
		return undefined;
	}

	function sendTypingState(state: "active" | "done") {
		const target = getTarget();
		if (!target) return;
		if (state === "active" && typingActive) return;
		if (state === "done" && !typingActive) return;
		typingActive = state === "active";
		void ircApiService.sendTyping({ target, state }).catch(() => {});
	}

	function onTyping() {
		sendTypingState("active");
		clearTimeout(typingTimer);
		typingTimer = setTimeout(() => sendTypingState("done"), TYPING_PAUSE_MS);
	}

	function stopTyping() {
		clearTimeout(typingTimer);
		sendTypingState("done");
	}

	onDestroy(() => clearTimeout(typingTimer));

	const placeholder = $derived(
		args.type === "server" ? "Raw IRC command (e.g. NAMES #channel)" : "Message (or /command)",
	);

	$effect(() => {
		if (args.replyTo && inputEl) {
			inputEl.focus();
		}
	});

	async function send() {
		const text = input.trim();
		if (!text) return;

		if (args.type === "server" || text.startsWith("/")) {
			const command = text.startsWith("/") ? text.slice(1) : text;
			await ircApiService.sendRaw({ command });
		} else {
			const target = args.type === "group" ? args.channel : args.nick;
			const replyTo = args.replyTo?.msgid;
			await ircApiService.sendMessage({ target, message: text, replyTo });
		}

		input = "";
		stopTyping();
		args.onSent?.();
	}
</script>

<div class="irc-input-area" class:replying={args.replyTo}>
	{#if args.replyTo}
		<div class="reply-indicator text-11">
			<span class="reply-text">
				Replying to {args.replyTo.sender}: {args.replyTo.content.length > 60
					? args.replyTo.content.slice(0, 60) + "..."
					: args.replyTo.content}
			</span>
			<button type="button" class="reply-cancel" onclick={args.onCancelReply}>x</button>
		</div>
	{/if}
	<div class="irc-input-wrapper">
		<textarea
			bind:this={inputEl}
			bind:value={input}
			{placeholder}
			class="irc-input text-13"
			rows="1"
			onkeydown={(e) => {
				if (e.key === "Enter" && !e.shiftKey) {
					e.preventDefault();
					send();
				}
				if (e.key === "Escape" && args.replyTo) args.onCancelReply?.();
			}}
			oninput={() => {
				if (inputEl) {
					inputEl.style.height = "auto";
					inputEl.style.height = Math.min(inputEl.scrollHeight, 120) + "px";
				}
				onTyping();
			}}
		></textarea>
		<div class="actions">
			<Button type="button" size="tag" style="pop" onclick={send}>send</Button>
		</div>
	</div>
</div>

<style lang="postcss">
	.irc-input-area {
		padding: 6px 8px;
		background-color: var(--clr-bg-muted);
	}
	.reply-indicator {
		display: flex;
		align-items: center;
		padding: 4px 8px;
		gap: 6px;
		border: 1px solid var(--clr-border-3);
		border-bottom: none;
		border-radius: var(--radius-s) var(--radius-s) 0 0;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		font-family: var(--font-mono);
	}
	.reply-text {
		flex-grow: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.reply-cancel {
		flex-shrink: 0;
		padding: 0 4px;
		color: var(--clr-text-3);
		cursor: pointer;
		&:hover {
			color: var(--clr-text-1);
		}
	}
	.irc-input-wrapper {
		display: flex;
		align-items: flex-end;
		min-height: 36px;
		gap: 8px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}
	.replying .irc-input-wrapper {
		border-radius: 0 0 var(--radius-s) var(--radius-s);
	}
	.irc-input {
		flex-grow: 1;
		min-height: 36px;
		max-height: 120px;
		padding: 8px 6px;
		border: none;
		outline: none;
		line-height: 1.4;
		font-family: inherit;
		resize: none;
	}
	.actions {
		display: flex;
		align-items: center;
		padding: 6px;
	}
</style>
