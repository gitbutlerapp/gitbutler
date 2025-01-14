<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import type { Snippet } from 'svelte';
	import { goto } from '$app/navigation';

	interface Props {
		children?: Snippet;
	}

	const { children }: Props = $props();
</script>

<Button
	kind="outline"
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
