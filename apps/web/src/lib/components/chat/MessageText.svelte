<script lang="ts">
	import Mention from './Mention.svelte';
	import { getChatMessageWords } from '$lib/chat/utils';
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
