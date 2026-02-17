<script lang="ts">
	import type { ExternallyResolvedPromise } from "$lib/utils/resolveExternally";

	type Props = {
		promise: ExternallyResolvedPromise<undefined>;
		log: (value: string) => void;
	};

	const { promise, log }: Props = $props();

	let value = $state("hello");

	async function logfn() {
		log(value);
		await promise.promise;
		log(value);
	}
</script>

<button onclick={logfn} type="button">log</button>
<button onclick={() => (value = "world")} type="button">update-state</button>
