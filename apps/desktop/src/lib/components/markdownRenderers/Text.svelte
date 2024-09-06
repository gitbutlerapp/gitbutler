<script lang="ts">
	interface Props {
		text: string;
	}

	const { text }: Props = $props();
</script>

<span>
	{text}
</span>
