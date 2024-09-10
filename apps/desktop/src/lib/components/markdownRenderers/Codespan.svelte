<script lang="ts">
	interface Props {
		raw: string;
	}

	const { raw = '' }: Props = $props();
</script>

<code>{raw.replace(/`/g, '')}</code>
