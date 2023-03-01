<script lang="ts">
	import type { Delta } from '$lib/deltas';
	import codeviewer from './codeviewer';

	export let doc: string;
	export let deltas: Delta[];
	export let end: number;
	export let filepath: string;
</script>

<code class="w-full h-full" use:codeviewer={{ doc, deltas, end, filepath }} />
