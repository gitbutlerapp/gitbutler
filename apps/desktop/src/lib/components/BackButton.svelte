<script lang="ts">
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { goto } from '$app/navigation';
	async function defaultBeforeOnMouseDown() {
		Promise.resolve();
	}

	export let beforeOnMouseDown: () => Promise<void> = defaultBeforeOnMouseDown;
</script>

<Button
	style="ghost"
	outline
	onmousedown={() => {
		beforeOnMouseDown().then(
			() => {
				if (history.length > 0) {
					history.back();
				} else {
					goto('/');
				}
			},
			(err) => {
				console.log('The pre-back button action failed');
				console.log(err);
			}
		);
	}}
>
	<slot />
</Button>
