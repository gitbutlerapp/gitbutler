<script lang="ts">
	interface Props {
		raw: string;
	}

	const { raw }: Props = $props();
</script>

{@html raw}
