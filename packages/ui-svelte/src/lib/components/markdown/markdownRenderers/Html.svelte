<script lang="ts">
	import DOMPurify from "isomorphic-dompurify";

	interface Props {
		raw: string;
	}

	const { raw }: Props = $props();

	const sanitized = $derived(
		DOMPurify.sanitize(raw, {
			ALLOWED_TAGS: [
				"b",
				"strong",
				"i",
				"em",
				"s",
				"del",
				"u",
				"mark",
				"sub",
				"sup",
				"small",
				"code",
				"pre",
				"h1",
				"h2",
				"h3",
				"h4",
				"h5",
				"h6",
				"p",
				"br",
				"hr",
				"blockquote",
				"ul",
				"ol",
				"li",
				"table",
				"thead",
				"tbody",
				"tr",
				"th",
				"td",
				"details",
				"summary",
			],
			ALLOWED_ATTR: [],
			ALLOWED_NAMESPACES: [],
			ALLOW_ARIA_ATTR: false,
			ALLOW_SELF_CLOSE_IN_ATTR: false,
			ALLOW_DATA_ATTR: false,
			ALLOW_UNKNOWN_PROTOCOLS: false,
		}),
	);
</script>

{@html sanitized}
