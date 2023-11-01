<script lang="ts">
	import type { Session } from '$lib/backend/sessions';
	import type { Delta } from '$lib/backend/deltas';
	import type { Frame } from './frame';
	import DeltasViewer from '$lib/components/DeltasViewer.svelte';

	export let context: number;
	export let fullContext: boolean;
	export let sessions: Session[];
	export let deltas: [string, Delta][][];
	export let files: Partial<Record<string, string>>[];
	export let value: number;
	export let frame: Frame | null = null;

	$: {
		if (deltas && files) {
			let i = value;
			for (const j in deltas) {
				const dd = deltas[j];
				if (i < dd.length) {
					const frameDeltas = dd.slice(0, i + 1);
					const frameFilepath = frameDeltas[frameDeltas.length - 1][0];
					frame = {
						sessionId: sessions[j].id,
						deltas: frameDeltas
							.filter((delta) => delta[0] === frameFilepath)
							.sort((a, b) => a[1].timestampMs - b[1].timestampMs)
							.map((delta) => delta[1]),
						filepath: frameFilepath,
						doc: files[j][frameFilepath] || ''
					};
					break;
				}
				i -= dd.length;
			}
		}
	}
</script>

{#if frame}
	<div id="code" class="overflow-auto">
		<DeltasViewer
			doc={frame.doc}
			deltas={frame.deltas}
			filepath={frame.filepath}
			paddingLines={fullContext ? 100000 : context}
		/>
	</div>
{:else}
	<div class="mt-8 text-center">Select a playlist</div>
{/if}
