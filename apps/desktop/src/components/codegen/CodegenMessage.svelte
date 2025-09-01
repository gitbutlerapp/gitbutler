<script lang="ts">
	import { Avatar, Icon, Markdown, AgentAvatar } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	type Props = {
		side: 'left' | 'right';
		content?: string;
		avatarUrl?: string;
		extraContent?: Snippet;
		bubble?: boolean;
	};

	const { side, content, avatarUrl, extraContent, bubble }: Props = $props();
</script>

<div class="message message-{side}" class:bubble>
	<div class="message-avatar">
		{#if avatarUrl}
			<Avatar size="large" srcUrl={avatarUrl} tooltip="" />
		{:else if side === 'right'}
			<div class="user-icon">
				<Icon name="profile" />
			</div>
		{:else}
			<AgentAvatar />
		{/if}
	</div>
	<div class="text-13 text-body message-content">
		{#if content}
			{#if bubble}
				<div class="message-user-bubble">
					<Markdown {content} />
					{#if extraContent}
						{@render extraContent()}
					{/if}
				</div>
			{:else}
				<Markdown {content} />
			{/if}
		{/if}

		{#if extraContent && !bubble}
			{@render extraContent()}
		{/if}
	</div>
</div>

<style lang="postcss">
	.message {
		display: flex;
		align-items: flex-end;
		width: 100%;
		padding: 8px 16px 16px 16px;

		&:not(.bubble) {
			gap: 12px;
		}

		&.bubble {
			gap: 8px;
		}
	}

	.message-left {
	}

	.message-right {
		justify-content: flex-end;
	}

	.message-user-bubble {
		padding: 10px 14px;
		border-radius: var(--radius-l);
		border-bottom-left-radius: 0;
		background-color: var(--clr-bg-2);
	}

	.message-content {
		display: flex;
		flex-direction: column;
		max-width: calc(100% - 40px);
		gap: 16px;
		text-wrap: wrap;
	}

	.user-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 16px;
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
	}
</style>
