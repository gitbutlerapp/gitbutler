<script lang="ts">
	import type { Interest } from '$lib/redux/interest';

	type Props = {
		interest: Interest;
		// ref?: HTMLElement;
	};

	const { interest }: Props = $props();

	$effect(() => {
		const unsubscribe = interest._subscribe();

		// It is vitally important that we return the unsubscribe function
		return unsubscribe;
	});
</script>
