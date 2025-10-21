<script lang="ts">
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		isWorkspace?: boolean;
		branchIcon?: Snippet;
		workspaceActions: Snippet;
		contextActions?: Snippet;
		messages: Snippet;
		input?: Snippet;
	};

	const {
		branchName,
		isWorkspace,
		branchIcon,
		workspaceActions,
		contextActions,
		messages,
		input
	}: Props = $props();
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<div class="chat-header" use:focusable>
		<div class="flex gap-10 justify-between overflow-hidden">
			{#if !isWorkspace}
				{@render branchIcon?.()}

				<div class="chat-header__title">
					<h3 class="text-15 text-bold truncate">{branchName}</h3>

					<div class="chat-header__title-actions">
						{@render workspaceActions()}
					</div>
				</div>
			{/if}
		</div>

		{#if contextActions}
			<div class="flex gap-8 overflow-hidden">
				{@render contextActions()}
			</div>
		{/if}
	</div>

	<div class="chat-container">
		{@render messages()}
	</div>

	{@render input?.()}
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
		justify-content: space-between;
		width: 100%;
		padding: 16px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.chat-header__title {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		gap: 8px;
	}

	.chat-header__title-actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.chat-container {
		display: flex;
		position: relative;
		flex: 1;
		flex-grow: 1;
		flex-direction: column;
		width: 100%;
		height: 100%;
		min-height: 10rem;
		overflow: hidden;
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
