import Link from '$lib/shared/Link.svelte';
import Blockquote from '$lib/utils/markdownRenderers/Blockquote.svelte';
import Code from '$lib/utils/markdownRenderers/Code.svelte';
import Codespan from '$lib/utils/markdownRenderers/Codespan.svelte';
import Heading from '$lib/utils/markdownRenderers/Heading.svelte';
import Image from '$lib/utils/markdownRenderers/Image.svelte';
import Space from '$lib/utils/markdownRenderers/Space.svelte';
import Text from '$lib/utils/markdownRenderers/Text.svelte';
import type { MarkedOptions } from 'marked';
import type { Component } from 'svelte';

export const renderers: Record<string, Component> = {
	link: Link,
	image: Image,
	space: Space,
	blockquote: Blockquote,
	code: Code,
	codespan: Codespan,
	text: Text,
	heading: Heading
};

export const options: MarkedOptions = {
	async: false,
	breaks: false,
	gfm: true,
	pedantic: false,
	renderer: null,
	silent: false,
	tokenizer: null,
	walkTokens: null
};
