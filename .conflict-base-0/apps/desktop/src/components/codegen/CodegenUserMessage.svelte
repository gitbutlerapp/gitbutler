<script lang="ts">
	import { Avatar, Icon, Markdown } from '@gitbutler/ui';

	interface Props {
		content?: string;
		avatarUrl?: string;
	}

	let { content, avatarUrl }: Props = $props();
</script>

<div class="message-user">
	{#if avatarUrl}
		<Avatar size="large" srcUrl={avatarUrl} tooltip="" />
	{:else}
		<div class="user-icon">
			<Icon name="profile" />
		</div>
	{/if}

	<div class="text-13 text-body message-bubble">
		<Markdown {content} />
	</div>
</div>

<style lang="postcss">
	.message-user {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		width: 100%;
		padding: 8px 0 16px;
		gap: 10px;
	}

	.message-bubble {
		display: flex;
		flex-direction: column;
		max-width: calc(var(--message-max-width) - 6%);
		padding: 10px 14px;
		gap: 16px;
		border-radius: var(--radius-ml);
		border-bottom-left-radius: 0;
		background-color: var(--clr-bg-2);
		text-wrap: wrap;
		word-break: break-word;
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
