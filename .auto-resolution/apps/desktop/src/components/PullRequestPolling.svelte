<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		number: number;
	};

	const { number }: Props = $props();
	const [forge] = inject(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);

	let elapsedMs: number | undefined = $state();
	let isClosed = $state(false);
	let pollCount = 0;

	let pollingInterval = $derived.by(() => {
		// If the PR is closed we update infrequently.
		if (isClosed) {
			return 30 * 60 * 1000;
		}
		// Right after creating a pull request it might not be returned
		// by the api for a few seconds. So we poll frequently but stop
		// after a while.
		if (!elapsedMs) {
			return pollCount < 5 ? 2000 : 0;
		}

		if (elapsedMs < 60 * 1000) {
			return 5 * 1000;
		} else if (elapsedMs < 10 * 60 * 1000) {
			return 30 * 1000;
		} else if (elapsedMs < 60 * 60 * 1000) {
			return 5 * 60 * 1000;
		}
		return 30 * 60 * 1000;
	});

	const prResult = $derived(prService?.get(number, { subscriptionOptions: { pollingInterval } }));

	$effect(() => {
		const result = prResult?.current;
		const pr = result?.data;

		if (pr) {
			const lastUpdatedMs = Date.parse(pr.updatedAt);
			isClosed = pr.state === 'closed';
			elapsedMs = Date.now() - lastUpdatedMs;
		}

		if (result?.isLoading) {
			pollCount += 1;
		}
	});
</script>
