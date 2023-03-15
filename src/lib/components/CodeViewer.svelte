<script lang="ts">
	import { type Delta, Operation } from '$lib/deltas';
	import { createTwoFilesPatch } from 'diff';

	import hljs from 'highlight.js';
	import 'highlight.js/styles/base16/gruvbox-dark-medium.css';

	import { parse } from 'diff2html';
	import type { DiffBlock, LineType } from 'diff2html/lib/types';
	import { onMount } from 'svelte';

	onMount(hljs.initHighlighting);

	export let doc: string;
	export let deltas: Delta[];
	export let filepath: string;

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

	const highlightBlocks = (blocks: DiffBlock[]) =>
		blocks.map(({ header, lines }) => ({
			header,
			lines: lines.map(({ oldNumber, newNumber, type, content }) => ({
				oldNumber,
				newNumber,
				type,
				prefix: content.substring(0, 1),
				originalContent: content.substring(1),
				content: hljs.highlight(content.substring(1), { language }).value
			}))
		}));

	const getLanguage = (filepath: string) => {
		const ext = filepath.split('.').pop();
		return ext && hljs.getLanguage(ext) ? ext : 'plaintext';
	};

	let editor: HTMLElement;

	$: left = deltas.length > 0 ? applyDeltas(doc, deltas.slice(0, deltas.length - 1)) : doc;
	$: right = deltas.length > 0 ? applyDeltas(left, deltas.slice(deltas.length - 1)) : left;
	$: patch = createTwoFilesPatch(filepath, filepath, left, right, '', '', { context: 100000 });
	$: language = getLanguage(filepath);
	$: parsed = parse(patch);
	$: parsed &&
		editor &&
		editor.querySelector('.changed')?.scrollIntoView({ block: 'center', behavior: 'smooth' });

	const bgColor = (type: LineType) =>
		type === 'insert' ? 'bg-[#14FF00]/20' : type === 'delete' ? 'bg-[#FF0000]/20' : '';
</script>

<table class="h-full w-full overflow-x-hidden whitespace-pre font-mono" bind:this={editor}>
	{#each parsed as hunk}
		<tbody>
			{#each highlightBlocks(hunk.blocks) as block}
				{#each block.lines as line}
					<tr>
						<td>
							<div class="flex select-none justify-between gap-2">
								<div>{line.oldNumber ?? ''}</div>
								<div>{line.newNumber ?? ''}</div>
							</div>
						</td>

						<td
							class={bgColor(line.type)}
							class:changed={line.type === 'insert' || line.type === 'delete'}
						>
							<div class="d2h-code-line relative px-4">
								{#if line.content}
									<span>{@html line.content}</span>
								{:else}
									<span class="d2h-code-line-ctn whitespace-pre">{line.originalContent}</span>
								{/if}
							</div>
						</td>
					</tr>
				{/each}
			{/each}
		</tbody>
	{/each}
</table>
