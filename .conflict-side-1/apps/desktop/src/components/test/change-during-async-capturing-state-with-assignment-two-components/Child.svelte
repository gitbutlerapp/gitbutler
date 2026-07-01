<script lang="ts">
	import type { ExternallyResolvedPromise } from "$lib/utils/resolveExternally";

	type Props = {
		promise: ExternallyResolvedPromise<undefined>;
		log: (value: string) => void;
		value: string;
	};

	const { promise, log, value }: Props = $props();

	async function logfn() {
		let value2 = value;
		log(value2);
		await promise.promise;
		log(value);
		log(value2);
	}
</script>

<button onclick={logfn} type="button">log</button>
