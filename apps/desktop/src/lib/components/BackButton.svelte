<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';
	interface Props {
		children?: import('svelte').Snippet;
	}

	let { children }: Props = $props();
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
