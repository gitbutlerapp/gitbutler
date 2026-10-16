<script lang="ts">
	import Link from "$components/Link.svelte";
	import Self from "$components/markdown/MarkdownContent.svelte";
	import Blockquote from "$components/markdown/markdownRenderers/Blockquote.svelte";
	import Br from "$components/markdown/markdownRenderers/Br.svelte";
	import Code from "$components/markdown/markdownRenderers/Code.svelte";
	import Codespan from "$components/markdown/markdownRenderers/Codespan.svelte";
	import Em from "$components/markdown/markdownRenderers/Em.svelte";
	import Heading from "$components/markdown/markdownRenderers/Heading.svelte";
	import Html from "$components/markdown/markdownRenderers/Html.svelte";
	import Image from "$components/markdown/markdownRenderers/Image.svelte";
	import List from "$components/markdown/markdownRenderers/List.svelte";
	import ListItem from "$components/markdown/markdownRenderers/ListItem.svelte";
	import Paragraph from "$components/markdown/markdownRenderers/Paragraph.svelte";
	import Separator from "$components/markdown/markdownRenderers/Separator.svelte";
	import Strong from "$components/markdown/markdownRenderers/Strong.svelte";
	import Table from "$components/markdown/markdownRenderers/Table.svelte";
	import Text from "$components/markdown/markdownRenderers/Text.svelte";
	import type { Token } from "marked";
	import type { Component } from "svelte";

	type Props = { type: "init"; tokens: Token[] } | Token;

	const renderers = {
		link: Link,
		image: Image,
		blockquote: Blockquote,
		code: Code,
		codespan: Codespan,
		text: Text,
		html: Html,
		list: List,
		list_item: ListItem,
		heading: Heading,
		paragraph: Paragraph,
		init: null,
		br: Br,
		strong: Strong,
		em: Em,
		hr: Separator,
		table: Table,
	};

	type TableToken = Extract<Token, { type: "table" }>;
	const { type, ...rest }: Props = $props();

	type ListToken = Extract<Token, { type: "list" }>;
</script>

{#if type === "init" && "tokens" in rest && rest.tokens}
	{#each rest.tokens as token}
		<Self {...token} />
	{/each}
{:else if renderers[type as keyof typeof renderers]}
	{@const CurrentComponent = renderers[type as keyof typeof renderers] as Component}
	{#if type === "list"}
		{@const listItems = (rest as ListToken).items}
		<CurrentComponent {...rest}>
			{#each listItems as item}
				<Self {...item} />
			{/each}
		</CurrentComponent>
	{:else if type === "table"}
		{@const tableToken = rest as TableToken}
		<CurrentComponent {...rest}>
			<thead>
				<tr>
					{#each tableToken.header as headerCell}
						<th>
							{#each headerCell.tokens as header}
								<Self {...header} />
							{/each}
						</th>
					{/each}
				</tr>
			</thead>
			<tbody>
				{#each tableToken.rows as row}
					<tr>
						{#each row as cell}
							<td>
								{#each cell.tokens as cellToken}
									<Self {...cellToken} />
								{/each}
							</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</CurrentComponent>
	{:else}
		<CurrentComponent {...rest}>
			{#if "tokens" in rest && rest.tokens}
				<Self type="init" tokens={rest.tokens} />
			{:else if "raw" in rest}
				{rest.raw}
			{/if}
		</CurrentComponent>
	{/if}
{/if}
