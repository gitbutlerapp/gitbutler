<script module lang="ts">
	import HunkDiff from '$lib/HunkDiff.svelte';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Code / HunkDiff',
		component: HunkDiff,
		args: {},
		argTypes: {}
	});

	const hunkStr = {
		diff: '@@ -140,7 +140,7 @@ \n <Modal\n \tbind:this={modal}\n \twidth="small"\n-\ttitle="Git fetch requires input"\n+\ttitle="Git --ds--tch requires input"\n…',
		newLines: 7,
		newStart: 140,
		oldLines: 7,
		oldStart: 140
	};
</script>

<script lang="ts">
	setTemplate(template);
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	<div class="wrap">
		<HunkDiff filePath="test.tsx" hunkStr={hunkStr.diff} {...args} />
	</div>
{/snippet}

<Story name="Playground" />

<style>
	.wrap {
		display: flex;
	}
</style>
