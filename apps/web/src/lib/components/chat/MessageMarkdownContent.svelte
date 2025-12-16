<script lang="ts">
	import MessageCode from '$lib/components/chat/MessageCode.svelte';
	import Self from '$lib/components/chat/MessageMarkdownContent.svelte';
	import MessageText from '$lib/components/chat/MessageText.svelte';

	import {
		Blockquote,
		Br,
		Codespan,
		Em,
		Heading,
		Html,
		Image,
		Link,
		List,
		ListItem,
		Paragraph,
		Strong
	} from '@gitbutler/ui';
	import type { UserSimple } from '@gitbutler/shared/users/types';
	import type { Token } from 'marked';
	import type { Component } from 'svelte';

	type Props = { type: 'init'; tokens: Token[]; mentions: UserSimple[] } | Token;

	const renderers = {
		link: Link,
		image: Image,
		blockquote: Blockquote,
		code: MessageCode,
		codespan: Codespan,
		text: MessageText,
		html: Html,
		list: List,
		list_item: ListItem,
		heading: Heading,
		paragraph: Paragraph,
		init: null,
		br: Br,
		strong: Strong,
		em: Em
	};

	const { type, ...rest }: Props = $props();
	const mentions = $derived('mentions' in rest ? rest.mentions : []);

	type ListToken = Extract<Token, { type: 'list' }>;
</script>

{#if type === 'init' && 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<Self {...token} {mentions} />
	{/each}
{:else if renderers[type as keyof typeof renderers]}
	{@const CurrentComponent = renderers[type as keyof typeof renderers] as Component}
	{#if type === 'list'}
		{@const listItems = (rest as ListToken).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				<Self {...item} {mentions} />
			{/each}
		</CurrentComponent>
	{:else if type === 'text' && 'raw' in rest}
		<MessageText text={rest.raw} {mentions} />
	{:else}
		<CurrentComponent {...rest}>
			{#if 'tokens' in rest && rest.tokens}
				<Self type="init" tokens={rest.tokens} {mentions} />
			{:else if 'raw' in rest}
				{rest.raw}
			{/if}
		</CurrentComponent>
	{/if}
{/if}
