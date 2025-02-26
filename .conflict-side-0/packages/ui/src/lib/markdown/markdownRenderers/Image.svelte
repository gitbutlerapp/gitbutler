<script lang="ts">
	interface Props {
		href?: string;
		title?: string;
		text?: string;
	}

	const { href = '', title = undefined, text = '' }: Props = $props();
</script>

<img src={href} {title} alt={text} />
