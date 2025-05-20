<script lang="ts">
	import type { ExternallyResolvedPromise } from '$lib/utils/resolveExternally';

	type Props = {
		promise: ExternallyResolvedPromise<undefined>;
		log: (value: string) => void;
		value: string;
	};

	const { promise, log, value }: Props = $props();

	async function logfn() {
		log(value);
		await promise.promise;
		log(value);
	}
</script>

<button onclick={logfn} type="button">log</button>
