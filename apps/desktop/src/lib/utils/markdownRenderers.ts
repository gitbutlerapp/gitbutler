import Blockquote from '$lib/components/markdownRenderers/Blockquote.svelte';
import Code from '$lib/components/markdownRenderers/Code.svelte';
import Codespan from '$lib/components/markdownRenderers/Codespan.svelte';
import Heading from '$lib/components/markdownRenderers/Heading.svelte';
import Image from '$lib/components/markdownRenderers/Image.svelte';
import Paragraph from '$lib/components/markdownRenderers/Paragraph.svelte';
import Space from '$lib/components/markdownRenderers/Space.svelte';
import Text from '$lib/components/markdownRenderers/Text.svelte';
import Link from '$lib/shared/Link.svelte';

export const renderers = {
	link: Link,
	image: Image,
	space: Space,
	blockquote: Blockquote,
	code: Code,
	codespan: Codespan,
	text: Text,
	heading: Heading,
	paragraph: Paragraph
};

export const options = {
	async: false,
	breaks: true,
	gfm: true,
	pedantic: false,
	renderer: null,
	silent: false,
	tokenizer: null,
	walkTokens: null
};
