<script>
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';
	/** @type {{children?: import('svelte').Snippet}} */
	let { children } = $props();
</script>

<Button
	style="ghost"
	outline
	onmousedown={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
>
	{@render children?.()}
</Button>
