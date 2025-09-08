<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
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

	// Export method to scroll to bottom
	export function scrollToBottom() {
		if (scrollableContainer?.scrollToBottom) {
			scrollableContainer.scrollToBottom();
		}
	}
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

	<ConfigurableScrollableContainer bind:this={scrollableContainer} childrenWrapHeight="100%">
		<div class="chat-messages hide-native-scrollbar">
			{@render messages()}
		</div>
	</ConfigurableScrollableContainer>

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
</style>
