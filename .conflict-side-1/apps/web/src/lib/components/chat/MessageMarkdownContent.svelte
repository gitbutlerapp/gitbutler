<script lang="ts">
	import MessageCode from './MessageCode.svelte';
	import MessageText from './MessageText.svelte';
	import Self from '$lib/components/chat/MessageMarkdownContent.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Blockquote from '@gitbutler/ui/markdown/markdownRenderers/Blockquote.svelte';
	import Br from '@gitbutler/ui/markdown/markdownRenderers/Br.svelte';
	import Codespan from '@gitbutler/ui/markdown/markdownRenderers/Codespan.svelte';
	import Em from '@gitbutler/ui/markdown/markdownRenderers/Em.svelte';
	import Heading from '@gitbutler/ui/markdown/markdownRenderers/Heading.svelte';
	import Html from '@gitbutler/ui/markdown/markdownRenderers/Html.svelte';
	import Image from '@gitbutler/ui/markdown/markdownRenderers/Image.svelte';
	import List from '@gitbutler/ui/markdown/markdownRenderers/List.svelte';
	import ListItem from '@gitbutler/ui/markdown/markdownRenderers/ListItem.svelte';
	import Paragraph from '@gitbutler/ui/markdown/markdownRenderers/Paragraph.svelte';
	import Strong from '@gitbutler/ui/markdown/markdownRenderers/Strong.svelte';
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
</script>

{#if type === 'init' && 'tokens' in rest && rest.tokens}
	{#each rest.tokens as token}
		<Self {...token} {mentions} />
	{/each}
{:else if renderers[type as keyof typeof renderers]}
	{@const CurrentComponent = renderers[type as keyof typeof renderers] as Component<
		Omit<Props, 'type'>
	>}
	{#if type === 'list'}
		{@const listItems = (rest as Extract<Props, { type: 'list' }>).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				{@const ChildComponent = renderers[item.type]}
				<ChildComponent {...item}>
					<Self type="init" tokens={item.tokens} {mentions} />
				</ChildComponent>
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
