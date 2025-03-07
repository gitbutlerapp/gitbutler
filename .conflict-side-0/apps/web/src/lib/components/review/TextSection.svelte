<script lang="ts">
	import type { TextSection } from '@gitbutler/shared/patches/types';

	interface Props {
		section: TextSection;
	}

	const { section }: Props = $props();

	function addSection(position: number | undefined) {
		console.log('addSection', position);
	}

	function editSection(code: string | undefined) {
		console.log('editSection', code);
	}

	function deleteSection(code: string | undefined) {
		console.log('deleteSection', code);
	}

	function moveSection(position: number | undefined, direction: number) {
		console.log('moveSection', position, direction);
	}

	function saveSection(code: string | undefined) {
		console.log('saveSection', code);
	}
</script>

<div class="right">
	<button type="button" class="action" onclick={() => addSection(section.position)}>add</button>
	[
	<button type="button" class="action" onclick={() => editSection(section.code)}>edit</button>] [
	<button type="button" class="action" onclick={() => deleteSection(section.code)}>del</button>] [
	<button type="button" class="action" onclick={() => moveSection(section.position, -1)}>up</button>
	<button type="button" class="action" onclick={() => moveSection(section.position, 1)}>down</button
	>
	]
</div>
<div class="editor edit-{section.code}">
	<textarea class="editing">{section.data}</textarea>
	<button type="button" onclick={() => saveSection(section.code)}>Save</button>
</div>
<div class="markdown display-{section.code}">{section.data}</div>

<style>
	.right {
		display: flex;
		flex-direction: row;
		justify-content: flex-end;
		gap: 5px;
		color: #888;
	}

	.action {
		cursor: pointer;
		color: #999;
	}

	.editing {
		width: 100%;
		height: 100px;
		font-family: monospace;
		font-size: large;
	}

	.editor {
		display: none;
	}
</style>
