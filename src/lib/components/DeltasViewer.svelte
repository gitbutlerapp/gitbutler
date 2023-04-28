<script lang="ts">
	import { type Delta, Operation } from '$lib/api';
	import { Differ } from '$lib/components';
	import { lineDiff } from './Differ/diff';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
	export let highlight: string[] = [];
	export let paddingLines = 10000;

	const applyDeltas = (text: string, deltas: Delta[]) => {
		const operations = deltas.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (Operation.isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (Operation.isDelete(operation)) {
				text =
					text.slice(0, operation.delete[0]) +
					text.slice(operation.delete[0] + operation.delete[1]);
			}
		});
		return text;
	};

	$: left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
	$: right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
	$: diff = lineDiff(left.split('\n'), right.split('\n'));
</script>

<Differ {diff} {filepath} {highlight} {paddingLines} />
