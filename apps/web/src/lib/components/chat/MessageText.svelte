<script lang="ts">
	import { getChatMessageWords } from '$lib/chat/mentions';
	import Mention from '$lib/components/chat/Mention.svelte';
	import type { UserSimple } from '@gitbutler/shared/users/types';

	interface Props {
		text: string;
		mentions: UserSimple[];
	}

	const { text, mentions }: Props = $props();

	const userMap = $derived(new Map(mentions.map((user) => [user.id, user])));
	const words = $derived(getChatMessageWords(text, userMap));
</script>

<span>
	{#each words as word}
		{#if word.type === 'text'}
			{word.value + ' '}
		{:else}
			<Mention mention={word.mention} />
			{' '}
		{/if}
	{/each}
</span>
