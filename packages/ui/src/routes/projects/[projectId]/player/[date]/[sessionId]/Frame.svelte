<script lang="ts">
	import type { Session } from '$lib/api/ipc/sessions';
	import type { Delta } from '$lib/api/ipc/deltas';
	import type { Frame } from './frame';
	import { DeltasViewer } from '$lib/components';
	import type { Readable } from '@square/svelte-store';
	import { Loaded, type Loadable } from 'svelte-loadable-store';

	export let context: number;
	export let fullContext: boolean;
	export let sessions: Session[];
	export let deltas: Readable<Loadable<[string, Delta][][]>>;
	export let files: Readable<Loadable<Partial<Record<string, string>>[]>>;
	export let value: number;
	export let frame: Frame | null = null;

	$: {
		if (
			!$deltas.isLoading &&
			!$files.isLoading &&
			Loaded.isValue($deltas) &&
			Loaded.isValue($files)
		) {
			let i = value;
			for (const j in $deltas.value) {
				const dd = $deltas.value[j];
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
						doc: $files.value[j][frameFilepath] || ''
					};
					break;
				}
				i -= dd.length;
			}
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
