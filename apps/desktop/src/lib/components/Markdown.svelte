<script lang="ts">
	import MarkdownContent from '$lib/components/MarkdownContent.svelte';

	interface Props {
		content?: string;
	}

	let { content }: Props = $props();
</script>

<div class="markdown-content">
	<MarkdownContent {content} />
</div>

<style>
	.markdown-content {
		display: inline;
	}
</style>
