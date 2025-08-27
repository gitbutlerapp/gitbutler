<script lang="ts">
	import Child from '$components/test/change-during-async-sees-updates-two-components/Child.svelte';
	import type { ExternallyResolvedPromise } from '$lib/utils/resolveExternally';

	type Props = {
		promise: ExternallyResolvedPromise<undefined>;
		log: (value: string) => void;
	};

	const { promise, log }: Props = $props();

	let value = $state('hello');
</script>

<Child {promise} {log} {value} />
<button onclick={() => (value = 'world')} type="button">update-state</button>
