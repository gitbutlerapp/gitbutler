<script lang="ts">
	import type { Interest } from '$lib/interest/intrestStore';

	type Props = {
		interest: Interest;
		reference?: HTMLElement;
		onlyInView?: boolean;
	};

	const { interest, reference: ref, onlyInView }: Props = $props();

	let inView = $state(false);

	$effect(() => {
		if (ref && onlyInView) {
			inView = false;

			const observer = new IntersectionObserver(
				(entries) => {
					inView = entries[0]?.isIntersecting || false;
				},
				{
					root: null
				}
			);

			observer.observe(ref);

			return () => {
				inView = false;
				observer.disconnect();
			};
		}
	});

	$effect(() => {
		if (!onlyInView || inView) {
			const unsubscribe = interest._subscribe();

			// It is vitally important that we return the unsubscribe function
			return unsubscribe;
		}
	});
</script>
