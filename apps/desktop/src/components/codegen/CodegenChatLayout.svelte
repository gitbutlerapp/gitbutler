<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
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

	let scrollableContainer: ConfigurableScrollableContainer;
	let scrollDistanceFromBottom = $state(0);

	// Export method to scroll to bottom
	export function scrollToBottom() {
		scrollableContainer?.scrollToBottom();
	}

	// Calculate distance from bottom when scrolling
	function handleScroll(event: Event) {
		const { scrollTop, scrollHeight, clientHeight } = event.target as HTMLElement;
		scrollDistanceFromBottom = scrollHeight - scrollTop - clientHeight;
	}

	// Show button only when user has scrolled more than 1000px from bottom
	const showScrollButton = $derived(scrollDistanceFromBottom > 600);
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<div class="chat-header" use:focusable>
		<div class="flex gap-10 items-center overflow-hidden">
			{@render branchIcon()}
			<p class="text-15 text-bold truncate">{branchName}</p>

			<div class="flex gap-4 items-center">
				{@render workspaceActions()}
			</div>
		</div>

		<div class="flex gap-4 items-center">
			{@render contextActions()}
		</div>
	</div>

	<div class="chat-container">
		<ConfigurableScrollableContainer
			bind:this={scrollableContainer}
			childrenWrapHeight="100%"
			onscroll={handleScroll}
		>
			<div class="chat-messages">
				{@render messages()}
			</div>
		</ConfigurableScrollableContainer>

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

	<div class="chat-footer" use:focusable>
		{@render input()}
	</div>
</div>

<style lang="postcss">
	.chat {
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
		justify-content: space-between;
		width: 100%;
		padding: 16px;
		gap: 20px;
		border-bottom: 1px solid var(--clr-border-3);
	}

	.chat-container {
		position: relative;
		flex: 1;
		overflow: hidden;
	}

	.chat-messages {
		display: flex;
		flex-direction: column;
		justify-content: flex-end;
		width: 100%;
		min-height: 100%;
		padding: 8px 20px;
	}

	.chat-footer {
		flex-shrink: 0;
		width: 100%;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
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
