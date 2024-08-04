<script lang="ts">
	import Button, { type ButtonProps } from '../../lib/Button.svelte';

	const props: ButtonProps = $props();
	const { children } = props;
</script>

<Button {...props}>{children}</Button>
