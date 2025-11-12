<script lang="ts">
	import PreviewHeader from '$components/PreviewHeader.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		inWorkspaceInlineContextActions?: Snippet;
		messages: Snippet;
		input?: Snippet;
		onclose?: () => void;
	};

	const { branchName, inWorkspaceInlineContextActions, messages, input, onclose }: Props = $props();
</script>

<div class="chat" use:focusable={{ vertical: true }}>
	<!-- TODO: remove this header when we move to the workspace layout -->
	<PreviewHeader {onclose}>
		{#snippet content()}
			<h3 class="text-14 text-semibold truncate">Chat for {branchName}</h3>
		{/snippet}

		{#snippet actions()}
			{@render inWorkspaceInlineContextActions?.()}
		{/snippet}
	</PreviewHeader>

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

	.chat-container {
		--message-max-width: 700px;
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
