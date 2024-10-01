import Blockquote from '$lib/components/markdownRenderers/Blockquote.svelte';
import Br from '$lib/components/markdownRenderers/Br.svelte';
import Code from '$lib/components/markdownRenderers/Code.svelte';
import Codespan from '$lib/components/markdownRenderers/Codespan.svelte';
import Heading from '$lib/components/markdownRenderers/Heading.svelte';
import Html from '$lib/components/markdownRenderers/Html.svelte';
import Image from '$lib/components/markdownRenderers/Image.svelte';
import List from '$lib/components/markdownRenderers/List.svelte';
import ListItem from '$lib/components/markdownRenderers/ListItem.svelte';
import Paragraph from '$lib/components/markdownRenderers/Paragraph.svelte';
import Strong from '$lib/components/markdownRenderers/Strong.svelte';
import Text from '$lib/components/markdownRenderers/Text.svelte';
import Link from '$lib/shared/Link.svelte';

export const renderers = {
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
	strong: Strong
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
