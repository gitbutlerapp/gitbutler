<script lang="ts">
	import type { Delta } from '$lib/api';
	import type { Frame } from './frame';
	import { DeltasViewer } from '$lib/components';

	export let context: number;
	export let fullContext: boolean;
	export let deltas: [string, Delta][][];
	export let files: Record<string, string>[];
	export let value: number;
	export let frame: Frame | null = null;

	$: {
		let i = value;
		for (const j in deltas) {
			const dd = deltas[j];
			if (i < dd.length) {
				const frameDeltas = dd.slice(0, i + 1);
				const frameFilepath = frameDeltas[frameDeltas.length - 1][0];
				frame = {
					deltas: frameDeltas
						.filter((delta) => delta[0] === frameFilepath)
						.map((delta) => delta[1]),
					filepath: frameFilepath,
					doc: files[j][frameFilepath] || ''
				};
				break;
			}
			i -= dd.length;
		}
	}
</script>

{#if frame}
	<div id="code" class="flex-auto overflow-auto bg-[#1E2021]">
		<div class="pb-[200px]">
			<DeltasViewer
				doc={frame.doc}
				deltas={frame.deltas}
				filepath={frame.filepath}
				paddingLines={fullContext ? 100000 : context}
			/>
		</div>
	</div>
{:else}
	<div class="mt-8 text-center">Select a playlist</div>
{/if}
