import Link from '$components/Link.svelte';
import Blockquote from '$components/markdownRenderers/Blockquote.svelte';
import Br from '$components/markdownRenderers/Br.svelte';
import Code from '$components/markdownRenderers/Code.svelte';
import Codespan from '$components/markdownRenderers/Codespan.svelte';
import Heading from '$components/markdownRenderers/Heading.svelte';
import Html from '$components/markdownRenderers/Html.svelte';
import Image from '$components/markdownRenderers/Image.svelte';
import List from '$components/markdownRenderers/List.svelte';
import ListItem from '$components/markdownRenderers/ListItem.svelte';
import Paragraph from '$components/markdownRenderers/Paragraph.svelte';
import Strong from '$components/markdownRenderers/Strong.svelte';
import Text from '$components/markdownRenderers/Text.svelte';

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
