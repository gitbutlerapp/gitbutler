<script lang="ts">
	import IrcChat from "$components/irc/IrcChat.svelte";
	import { BACKEND } from "$lib/backend";
	import ResizeHandles from "$lib/floating/ResizeHandles.svelte";
	import { IRC_CONNECTION_ID } from "$lib/irc/ircApiService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button } from "@gitbutler/ui";
	import { portal } from "@gitbutler/ui/utils/portal";
	import { onDestroy, onMount } from "svelte";

	const { projectId }: { projectId: string } = $props();
	const uiState = inject(UI_STATE);
	const backend = inject(BACKEND);

	const ircChatOpen = uiState.global.ircChatOpen;
	const channel = uiState.global.channel;

	// ── Open chat on mention ──
	const unlistenMention = backend.listen<{ from: string; target: string; text?: string }>(
		`irc:${IRC_CONNECTION_ID}:mention`,
		(event) => {
			const { target: mentionTarget } = event.payload;

			// If the chat is already open on this channel, nothing to do.
			if (ircChatOpen.current && channel.current === mentionTarget) return;

			// Switch to the relevant channel and open the chat.
			channel.set(mentionTarget);
			ircChatOpen.set(true);
		},
	);

	const MIN_WIDTH = 360;
	const MIN_HEIGHT = 300;
	const HEADER_HEIGHT = 32;
	const MIN_HEADER_VISIBLE = 120;

	let width = $state(uiState.global.ircChatSize.current.width);
	let height = $state(uiState.global.ircChatSize.current.height);
	let x = $state(0);
	let y = $state(0);
	let rehydrated = false;
	let defaultApplied = false;

	// Watch for rehydrated position (redux-persist may load async)
	$effect(() => {
		const savedXY = uiState.global.ircChatXY.current;
		const savedSize = uiState.global.ircChatSize.current;
		if (!rehydrated && savedXY) {
			x = savedXY.x;
			y = savedXY.y;
			width = savedSize.width;
			height = savedSize.height;
			rehydrated = true;
			clampPosition();
		}
	});

	function onWindowResize() {
		clampPosition();
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === "Escape" && ircChatOpen.current) {
			e.stopPropagation();
			ircChatOpen.set(false);
		}
	}

	onMount(() => {
		if (!rehydrated && !defaultApplied) {
			x = window.innerWidth - width - 20;
			y = window.innerHeight - height - 40;
			defaultApplied = true;
		}
		clampPosition();
		window.addEventListener("resize", onWindowResize);
		window.addEventListener("keydown", onKeydown, true);
	});

	onDestroy(() => {
		window.removeEventListener("resize", onWindowResize);
		window.removeEventListener("keydown", onKeydown, true);
		unlistenMention();
	});

	function clampPosition() {
		x = Math.max(MIN_HEADER_VISIBLE - width, Math.min(window.innerWidth - MIN_HEADER_VISIBLE, x));
		y = Math.max(0, Math.min(window.innerHeight - HEADER_HEIGHT, y));
	}

	function saveSize() {
		uiState.global.ircChatSize.set({ width, height });
	}

	function savePosition() {
		uiState.global.ircChatXY.set({ x, y });
	}

	// ── Drag ──────────────────────────────────────────────────────────────────

	let dragging = $state(false);
	let dragOffsetX = 0;
	let dragOffsetY = 0;

	function onDragStart(e: PointerEvent) {
		const target = e.target as HTMLElement;
		if (target.closest("[data-no-drag], button, input, textarea")) return;
		dragging = true;
		dragOffsetX = e.clientX - x;
		dragOffsetY = e.clientY - y;
		window.addEventListener("pointermove", onDragMove);
		window.addEventListener("pointerup", onDragEnd, { once: true });
		e.preventDefault();
	}

	function onDragMove(e: PointerEvent) {
		x = e.clientX - dragOffsetX;
		y = e.clientY - dragOffsetY;
		clampPosition();
	}

	function onDragEnd() {
		dragging = false;
		window.removeEventListener("pointermove", onDragMove);
		savePosition();
	}

	// ── Resize ────────────────────────────────────────────────────────────────

	let resizeDir = "";
	let resizeStartX = 0;
	let resizeStartY = 0;
	let resizeStartW = 0;
	let resizeStartH = 0;
	let resizeStartPosX = 0;
	let resizeStartPosY = 0;

	function onResizeStart(e: PointerEvent, direction: string) {
		resizeDir = direction;
		resizeStartX = e.clientX;
		resizeStartY = e.clientY;
		resizeStartW = width;
		resizeStartH = height;
		resizeStartPosX = x;
		resizeStartPosY = y;
		window.addEventListener("pointermove", onResizeMove);
		window.addEventListener("pointerup", onResizeEnd, { once: true });
		e.preventDefault();
	}

	function onResizeMove(e: PointerEvent) {
		const dx = e.clientX - resizeStartX;
		const dy = e.clientY - resizeStartY;

		let newW = resizeStartW;
		let newH = resizeStartH;
		let newX = resizeStartPosX;
		let newY = resizeStartPosY;

		if (resizeDir.includes("e")) newW = Math.max(MIN_WIDTH, resizeStartW + dx);
		if (resizeDir.includes("s")) newH = Math.max(MIN_HEIGHT, resizeStartH + dy);
		if (resizeDir.includes("w")) {
			newW = Math.max(MIN_WIDTH, resizeStartW - dx);
			newX = resizeStartPosX + resizeStartW - newW;
		}
		if (resizeDir.includes("n")) {
			newH = Math.max(MIN_HEIGHT, resizeStartH - dy);
			newY = resizeStartPosY + resizeStartH - newH;
			if (newY < 0) {
				newH += newY;
				newY = 0;
			}
		}

		width = newW;
		height = newH;
		x = newX;
		y = newY;
	}

	function onResizeEnd() {
		window.removeEventListener("pointermove", onResizeMove);
		saveSize();
		savePosition();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
{#if ircChatOpen.current}
	<div
		class="irc-floating-window"
		class:irc-dragging={dragging}
		style="left: {x}px; top: {y}px; width: {width}px; height: {height}px;"
		use:portal={"body"}
		onpointerdown={onDragStart}
	>
		<ResizeHandles snapPosition="" {onResizeStart} />

		<IrcChat {projectId}>
			{#snippet headerActions()}
				<Button size="tag" icon="cross" kind="ghost" onclick={() => ircChatOpen.set(false)} />
			{/snippet}
		</IrcChat>
	</div>
{/if}

<style lang="postcss">
	.irc-floating-window {
		display: flex;
		z-index: var(--z-floating);
		position: fixed;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background: var(--bg-1);
		box-shadow: var(--fx-shadow-l);
	}
</style>
