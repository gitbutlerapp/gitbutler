<script lang="ts">
	import type { Delta } from '$lib/deltas';
	import codeviewer from './action';

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;
</script>

<code class="h-full w-full" use:codeviewer={{ doc, deltas, filepath }} />
