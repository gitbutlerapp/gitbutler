<script lang="ts">
	import { parse } from '$lib/diff';
	import { Differ } from '$lib/components';

	export let diff: string;
	export let path: string;
	export let paddingLines = 3;

	$: parsedDiff = parse(diff);
</script>

<Differ diff={parsedDiff} filepath={path} {paddingLines} />
