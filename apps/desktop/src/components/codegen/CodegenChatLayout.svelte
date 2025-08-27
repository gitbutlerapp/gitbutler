<script lang="ts">
	import { focusable } from '$lib/focus/focusable.svelte';
	import { Icon } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;

		workspaceActions: Snippet;
		contextActions: Snippet;
		messages: Snippet;
		input: Snippet;
	};

	const { branchName, workspaceActions, contextActions, messages, input }: Props = $props();
</script>

<div class="chat" use:focusable={{ list: true }}>
	<div class="chat-header">
		<div class="chat-header-section">
			<div class="flex gap-8 items-center">
				<Icon name="branch-remote" />
				<p class="text-14 text-bold">{branchName}</p>
			</div>
			<div class="flex gap-4 items-center">
				{@render workspaceActions()}
			</div>
		</div>
		<div class="chat-header-section">
			<div class="flex gap-4 items-center">
				{@render contextActions()}
			</div>
		</div>
	</div>
	<div class="chat-messages">
		<div>
			{@render messages()}
		</div>
	</div>
	<div class="chat-footer">
		{@render input()}
	</div>
</div>

<style lang="postcss">
	.chat {
		display: flex;
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

		border-bottom: 1px solid var(--clr-border-3);
	}

	.chat-header-section {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.chat-messages {
		display: flex;
		flex-grow: 1;
		flex-direction: column-reverse;
		width: 100%;

		overflow: auto;
	}

	.chat-footer {
		flex-shrink: 0;
		width: 100%;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
