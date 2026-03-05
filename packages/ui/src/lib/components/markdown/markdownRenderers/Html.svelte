<script lang="ts">
	import DOMPurify from "isomorphic-dompurify";

	interface Props {
		raw: string;
	}

	const { raw }: Props = $props();

	const sanitized = $derived(DOMPurify.sanitize(raw));
</script>

{@html sanitized}
