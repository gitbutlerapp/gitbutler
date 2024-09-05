<script lang="ts">
	interface Props {
		text: string;
		lang: string;
	}

	const { text, lang }: Props = $props();
</script>

<pre class={lang}><code>{text}</code></pre>
