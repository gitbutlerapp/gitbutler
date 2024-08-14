<script lang="ts">
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { goto } from '$app/navigation';
	async function defaultPreMouseDown() {
		Promise.resolve();
	}

	export let preMouseDown: () => Promise<void> = defaultPreMouseDown;
</script>

<Button
	style="ghost"
	outline
	onmousedown={() => {
		preMouseDown().then(
			() => {
				if (history.length > 0) {
					history.back();
				} else {
					goto('/');
				}
			},
			(err) => {
				console.log('Failed to execute the pre-mouse-down action');
				console.log(err);
			}
		);
	}}
>
	<slot />
</Button>
