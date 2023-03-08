<script lang="ts">
	import type { Delta } from '$lib/deltas';
	import codeviewer from './action';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
	export let highlightLatest = false;
</script>

<code
	style:color-scheme="dark"
	class="h-full w-full"
	use:codeviewer={{ doc, deltas, filepath, highlightLatest }}
/>
