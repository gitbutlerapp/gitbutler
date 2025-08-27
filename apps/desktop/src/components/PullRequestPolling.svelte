<script lang="ts">
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { getPollingInterval } from '$lib/forge/shared/progressivePolling';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		number: number;
	};

	const { number }: Props = $props();
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const prService = $derived(forge.current.prService);

	let elapsedMs = $state<number>(0);
	let isClosed = $state(false);

	let pollingInterval = $derived(getPollingInterval(elapsedMs, isClosed));

	const prResult = $derived(prService?.get(number, { subscriptionOptions: { pollingInterval } }));

	$effect(() => {
		const result = prResult?.current;
		const pr = result?.data;

		if (pr) {
			const lastUpdatedMs = Date.parse(pr.updatedAt);
			isClosed = pr.state === 'closed';
			elapsedMs = Date.now() - lastUpdatedMs;
		}
	});
</script>
