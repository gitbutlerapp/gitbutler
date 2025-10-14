<script lang="ts">
	import { splitMessage } from '$lib/utils/commitMessage';
	import { MarkdownContent, TestId, Tooltip } from '@gitbutler/ui';
	import { Lexer } from 'marked';

	type Props = {
		truncate?: boolean;
		commitMessage: string;
		className?: string;
		editable?: boolean;
		rewrap?: boolean;
	};

	const { commitMessage, truncate, className, editable, rewrap }: Props = $props();

	const title = $derived(splitMessage(commitMessage).title);

	const markdownOptions = {
		async: false,
		breaks: true,
		gfm: true,
		pedantic: false,
		renderer: null,
		silent: false,
		tokenizer: null,
		walkTokens: null
	};

	const tokens = $derived.by(() => {
		if (!title) return [];
		const lexer = new Lexer(markdownOptions);
		return lexer.lex(title);
	});

	function getTitle() {
		if (title) {
			return title;
		}
		return editable ? 'Empty commit. Drag changes here' : 'Empty commit';
	}
</script>

<Tooltip text={getTitle()}>
	<h3
		data-testid={TestId.CommitDrawerTitle}
		class="{className} commit-title"
		class:commit-title-markdown={rewrap}
		class:truncate
		class:clr-text-3={!title}
	>
		{#if title && tokens.length > 0 && rewrap}
			<MarkdownContent type="init" {tokens} />
		{:else}
			{getTitle()}
		{/if}
	</h3>
</Tooltip>

<style>
	/* Make paragraphs inline in commit titles to avoid invalid HTML nesting */
	:global(.commit-title-markdown p) {
		display: inline;
		margin: 0;
	}

	/* Tone down markdown headers in titles */
	:global(.commit-title-markdown h1),
	:global(.commit-title-markdown h2),
	:global(.commit-title-markdown h3),
	:global(.commit-title-markdown h4) {
		display: inline;
		font-size: inherit !important;
		font-weight: 600;
		margin: 0;
	}
</style>
