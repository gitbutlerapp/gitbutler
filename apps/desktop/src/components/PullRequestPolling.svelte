<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/storeUtils';

	type Props = {
		number: number;
	};

	const { number }: Props = $props();
	const [forge] = inject(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);

	let lastUpdatedMs: number | undefined = $state();

	let pollingInterval = $derived.by(() => {
		if (!lastUpdatedMs) {
			return 5000;
		}
		const elapsedMs = Date.now() - lastUpdatedMs;
		if (elapsedMs < 60 * 1000) {
			return 5 * 1000;
		} else if (elapsedMs < 10 * 60 * 1000) {
			return 30 * 1000;
		} else if (elapsedMs < 60 * 60 * 1000) {
			return 5 * 60 * 1000;
		}
		return 30 * 60 * 1000;
	});

	const subscriptionOptions = reactive(() => ({ pollingInterval: pollingInterval }));

	const prResult = $derived.by(() => {
		return prService?.get(number, subscriptionOptions);
	});
	$inspect(prResult);
</script>
