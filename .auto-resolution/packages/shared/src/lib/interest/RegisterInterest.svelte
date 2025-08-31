<script lang="ts">
	import { registerInterest } from '$lib/interest/registerInterestFunction.svelte';
	import type { Interest } from '$lib/interest/interestStore';

	type Props = {
		interest: Interest;
		reference?: HTMLElement;
		onlyInView?: boolean;
	};

	const { interest, reference: element, onlyInView }: Props = $props();

	$effect(() => {
		if (onlyInView) {
			registerInterest(interest, { element });
		} else {
			registerInterest(interest);
		}
	});
</script>
