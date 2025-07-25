<script module lang="ts">
	import HunkDiff from '$lib/HunkDiff.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const hunkStr = {
		diff: '@@ -140,7 +140,7 @@ \n <Modal\n \tbind:this={modal}\n \twidth="small"\n-\ttitle="Git fetch requires input"\n+\ttitle="Git --ds--tch requires input"\n…<div class="modal-body">\n \t\t<p>Git fetch requires input</p>\n \t</div>\n </Modal>\n',
		newLines: 7,
		newStart: 140,
		oldLines: 7,
		oldStart: 140,
		linesAdded: 1,
		linesRemoved: 1,
		linesChanged: 2
	};

	const { Story } = defineMeta({
		title: 'Code / HunkDiff',
		component: HunkDiff,
		args: {
			hunkStr: hunkStr.diff,
			filePath: 'test.tsx'
		},
		argTypes: {}
	});
</script>

<Story name="default">
	{#snippet template(args)}
		<div class="wrap">
			<HunkDiff {...args} />
		</div>
	{/snippet}
</Story>

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
	}
</style>
