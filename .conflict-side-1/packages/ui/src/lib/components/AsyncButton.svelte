<script lang="ts">
	import Button, { type Props as ButtonProps } from '$components/Button.svelte';

	type Props = Omit<ButtonProps, 'onclick'> & {
		action: () => Promise<void>;
		stopPropagation?: boolean;
	};

	let { action, stopPropagation = false, ...rest }: Props = $props();

	let loading = $state(false);

	async function performAction(event: Event) {
		if (stopPropagation) {
			event.stopPropagation();
		}

		loading = true;

		try {
			await action();
		} finally {
			loading = false;
		}
	}
</script>

<Button onclick={performAction} {loading} {...rest}></Button>
