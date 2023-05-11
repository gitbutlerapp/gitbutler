<script lang="ts">
	import { parseDiff } from '$lib/components/Differ/diff';
	import { Differ } from '$lib/components';

	export let diff: string;
	export let path: string;
	export let paddingLines = 3;

	$: parsedDiff = parseDiff(diff);
</script>

<Differ diff={parsedDiff} filepath={path} {paddingLines} />
