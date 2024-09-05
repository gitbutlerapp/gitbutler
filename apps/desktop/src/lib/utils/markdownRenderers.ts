import Link from '$lib/shared/Link.svelte';
import Image from '$lib/utils/markdownRenderers/Image.svelte';
import Space from '$lib/utils/markdownRenderers/Space.svelte';

export const defaultRenderers = {
	link: Link,
	image: Image,
	space: Space
};

export const defaultOptions = {
	baseUrl: null,
	breaks: false,
	gfm: true,
	headerIds: true,
	headerPrefix: '',
	highlight: null,
	langPrefix: 'language-',
	mangle: true,
	pedantic: false,
	renderer: null,
	sanitize: false,
	sanitizer: null,
	silent: false,
	smartLists: false,
	smartypants: false,
	tokenizer: null,
	xhtml: false
};
