<script lang="ts">
	import { Avatar, Markdown } from '@gitbutler/ui';
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

<div class="message message-{side}">
	<div class="message-avatar">
		{#if avatarUrl}
			<Avatar size="large" srcUrl={avatarUrl} tooltip="" />
		{:else}
			{@render happyPC()}
		{/if}
	</div>
	<div class="message-content">
		{#if content}
			{#if bubble}
				<div class="message-user-bubble">
					<Markdown {content} />
				</div>
			{:else}
				<Markdown {content} />
			{/if}
		{/if}
		{#if extraContent}
			{@render extraContent()}
		{/if}
	</div>
</div>

{#snippet happyPC()}
	<svg width="30" height="32" viewBox="0 0 30 32" fill="none" xmlns="http://www.w3.org/2000/svg">
		<path
			d="M0.999023 12.3789C0.999023 9.0652 3.68532 6.37891 6.99902 6.37891H18.4527C21.7664 6.37891 24.4527 9.0652 24.4527 12.3789V15.3964C24.4527 17.1472 25.2175 18.8107 26.5464 19.9506L27.3212 20.6152C28.2072 21.3751 28.717 22.4841 28.717 23.6513V27.0011C28.717 29.2103 26.9262 31.0011 24.717 31.0011H6.99902C3.68532 31.0011 0.999023 28.3148 0.999023 25.0011V12.3789Z"
			fill="#F2F2DA"
			stroke="#C3C39F"
			stroke-width="1.2"
		/>
		<rect
			x="4.12793"
			y="9.45605"
			width="16.6801"
			height="18.4667"
			rx="4"
			fill="white"
			stroke="black"
			stroke-width="1.2"
		/>
		<path
			d="M7.54785 21.6074C11.2661 24.1184 13.293 24.1066 16.9027 21.6074"
			stroke="black"
			stroke-width="1.2"
		/>
		<rect x="8.2998" y="12.875" width="2.74194" height="6.57575" rx="1.37097" fill="black" />
		<rect x="13.9121" y="12.877" width="2.74194" height="6.57575" rx="1.37097" fill="black" />
		<path
			d="M21.1127 0C21.1127 4.92872 25.0916 8.92424 29.9998 8.92424C25.0916 8.92424 21.1127 12.9198 21.1127 17.8485C21.1127 12.9198 17.1338 8.92424 12.2256 8.92424C17.1338 8.92424 21.1127 4.92872 21.1127 0Z"
			fill="#24B4AD"
		/>
	</svg>
{/snippet}

<style lang="postcss">
	.message {
		display: flex;

		align-items: flex-end;
		width: 100%;
		padding: 8px 16px 16px 16px;
		gap: 8px;
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
</style>
