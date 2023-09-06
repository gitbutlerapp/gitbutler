<script lang="ts">
	import { isInsert, type Delta, isDelete } from '$lib/api/ipc/deltas';
	import Differ from './Differ';
	import { line } from '$lib/diff';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
	export let highlight: string[] = [];
	export let paddingLines = 10000;

	const applyDeltas = (text: string, deltas: Delta[]) => {
		const operations = deltas.flatMap((delta) => delta.operations);

		operations.forEach((operation) => {
			if (isInsert(operation)) {
				text =
					text.slice(0, operation.insert[0]) +
					operation.insert[1] +
					text.slice(operation.insert[0]);
			} else if (isDelete(operation)) {
				text =
					text.slice(0, operation.delete[0]) +
					text.slice(operation.delete[0] + operation.delete[1]);
			}
		});
		return text;
	};

	$: left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
	$: right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
	$: diff = line(left.split('\n'), right.split('\n'));
</script>

<Differ {diff} {filepath} {highlight} {paddingLines} />
