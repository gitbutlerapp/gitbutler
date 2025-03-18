<script lang="ts">
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';

	type Props = {
		number: number;
	};

	const { number }: Props = $props();
	const [forge] = inject(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);

	let lastUpdatedStr: string | undefined = $state();

	let pollingInterval = $derived.by(() => {
		if (!lastUpdatedStr) {
			return 5000;
		}
		const lastUpdatedMs = Date.parse(lastUpdatedStr);
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

	const prResult = $derived(prService?.get(number, { pollingInterval }));

	$effect(() => {
		const updatedAtStr = prResult?.current.data?.updatedAt;
		if (updatedAtStr) {
			lastUpdatedStr = updatedAtStr;
		}
	});
</script>
