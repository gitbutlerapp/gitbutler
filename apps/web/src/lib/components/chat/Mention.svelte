<script lang="ts">
	import { isMentionMatch, type MentionMatch } from '$lib/chat/mentions';
	import Self from '$lib/components/chat/Mention.svelte';

	interface Props {
		mention: MentionMatch;
	}

	const { mention }: Props = $props();

	const username = $derived(mention.user.login ?? mention.user.name ?? mention.user.email);
</script>

{#if isMentionMatch(mention.prefix)}
	<Self mention={mention.prefix} />
{:else}
	{mention.prefix}
{/if}
<span class="message-mention text-13">
	@{username}
</span>
{#if isMentionMatch(mention.suffix)}
	<Self mention={mention.suffix} />
{:else}
	{mention.suffix}
{/if}

<style>
	.message-mention {
		padding: 0px 4px;
		gap: 10px;
		border-radius: var(--radius-s, 4px);
		background: var(--clr-theme-pop-bg-muted, #e7f8f7);

		color: var(--clr-theme-pop-on-soft, #1c5451);
		font-style: normal;
	}
</style>
