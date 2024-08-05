<script lang="ts">
	import Button, { type ButtonProps } from '../../lib/Button.svelte';

	const props: ButtonProps = $props();
</script>

<Button {...props}>Button</Button>
