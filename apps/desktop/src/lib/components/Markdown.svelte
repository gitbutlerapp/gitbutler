<script lang="ts">
	import { defaultRenderers, defaultOptions } from '$lib/utils/markdownRenderers';
	import { Lexer, marked, type Token } from 'marked';

	interface Props {
		content?: string;
		type?: string;
		tokens?: Token[];
	}

	let { content, type, tokens, ...rest }: Props = $props();

	const lexer = new Lexer(defaultOptions);
	if (!tokens && content) {
		tokens = lexer.lex(content);
	}

	$inspect('TOKENS', tokens);
</script>

{#if !type && tokens}
	{#each tokens as token}
		<svelte:self {...token} />
	{/each}
{:else if type && defaultRenderers[type]}
	<svelte:component this={defaultRenderers[type]} {...rest} options={defaultOptions}>
		{#if tokens}
			<svelte:self {tokens} />
		{/if}
	</svelte:component>
{:else if tokens}
	<svelte:self {tokens} />
{:else}
	{@html marked.parse(rest.raw)}
{/if}
