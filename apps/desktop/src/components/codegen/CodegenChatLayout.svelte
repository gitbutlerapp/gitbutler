<script lang="ts">
	import PreviewHeader from '$components/PreviewHeader.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		isWorkspace?: boolean;
		branchIcon?: Snippet;
		pageWorkspaceActions: Snippet;
		pageContextActions?: Snippet;
		inWorkspaceInlineContextActions?: Snippet;
		messages: Snippet;
		input?: Snippet;
		onclose?: () => void;
	};

	const {
		branchName,
		isWorkspace,
		branchIcon,
		pageWorkspaceActions,
		pageContextActions,
		inWorkspaceInlineContextActions,
		messages,
		input,
		onclose
	}: Props = $props();
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<!-- TODO: remove this header when we move to the workspace layout -->
	{#if !isWorkspace}
		<div class="chat-header" use:focusable>
			<div class="flex gap-10 justify-between overflow-hidden">
				{@render branchIcon?.()}

				<div class="chat-header__title">
					<h3 class="text-15 text-bold truncate">{branchName}</h3>

					<div class="chat-header__title-actions">
						{@render pageWorkspaceActions()}
					</div>
				</div>
			</div>

			{#if pageContextActions}
				<div class="flex gap-8 overflow-hidden">
					{@render pageContextActions()}
				</div>
			{/if}
		</div>
	{:else}
		<PreviewHeader {onclose}>
			{#snippet content()}
				<h3 class="text-14 text-semibold truncate">Chat for {branchName}</h3>
			{/snippet}

			{#snippet actions()}
				{@render inWorkspaceInlineContextActions?.()}
			{/snippet}
		</PreviewHeader>
	{/if}

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
		--message-max-width: 620px;
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
</style>
