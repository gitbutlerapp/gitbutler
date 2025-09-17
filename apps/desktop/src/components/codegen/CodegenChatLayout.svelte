<script lang="ts">
	import { Button } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { fade } from 'svelte/transition';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		branchIcon: Snippet;
		workspaceActions: Snippet;
		contextActions: Snippet;
		messages: Snippet;
		input: Snippet;
	};

	const { branchName, branchIcon, workspaceActions, contextActions, messages, input }: Props =
		$props();

	let scrollDistanceFromBottom = $state(0);
	let bottomAnchor = $state<HTMLDivElement>();

	// Export method to scroll to bottom
	function scrollToBottom() {
		bottomAnchor?.scrollIntoView({ behavior: 'smooth' });
	}

	// Calculate distance from bottom when scrolling
	function handleScroll(event: Event) {
		const { scrollTop } = event.target as HTMLElement;
		scrollDistanceFromBottom = -scrollTop;
	}

	// Show button only when user has scrolled more than 1000px from bottom
	const showScrollButton = $derived(scrollDistanceFromBottom > 600);
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<div class="chat-header" use:focusable>
		<div class="flex gap-10 justify-between">
			<div class="chat-header__title">
				{@render branchIcon()}
				<p class="text-15 text-bold truncate">{branchName}</p>
			</div>
			<div class="flex gap-4 items-center">
				{@render contextActions()}
			</div>
		</div>

		<div class="chat-header__actions">
			{@render workspaceActions()}
		</div>
	</div>

	<div class="chat-container">
		<div class="chat-messages" onscroll={handleScroll}>
			<div bind:this={bottomAnchor} style="height: 1px; margin-top: 8px;"></div>
			{@render messages()}
		</div>

		{#if showScrollButton}
			<div class="chat-scroll-to-bottom" transition:fade={{ duration: 150 }}>
				<Button
					kind="outline"
					icon="arrow-down"
					tooltip="Scroll to bottom"
					onclick={scrollToBottom}
				/>
			</div>
		{/if}
	</div>

	{@render input()}
</div>

<style lang="postcss">
	.chat {
		container-name: chat;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.chat-header {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-3);
	}

	.chat-header__title {
		display: flex;
		flex: 1;
		align-items: center;
		margin-top: -2px;
		overflow: hidden;
		gap: 10px;
	}

	.chat-header__actions {
		display: flex;
		align-items: center;

		gap: 4px;
	}

	.chat-container {
		display: flex;
		position: relative;
		flex: 1;
		overflow: hidden;
	}

	.chat-messages {
		display: flex;
		flex-direction: column-reverse;
		width: 100%;
		padding: 8px 20px;
		overflow-x: hidden;
		overflow-y: scroll;
		scrollbar-width: none; /* Firefox */
		-ms-overflow-style: none; /* IE 10+ */
	}

	.chat-scroll-to-bottom {
		z-index: var(--z-floating);
		position: absolute;
		right: 16px;
		bottom: 14px;
		overflow: hidden;
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-1);
		transition:
			box-shadow var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			transform: scale(1.05) translateY(-2px);
			box-shadow: var(--fx-shadow-s);
		}
	}
</style>
