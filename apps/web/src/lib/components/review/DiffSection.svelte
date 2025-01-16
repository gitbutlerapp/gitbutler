<script lang="ts">
	import type { DiffSection } from '@gitbutler/shared/branches/types';

	interface Props {
		section: DiffSection;
	}

	const { section }: Props = $props();

	function addSection(position: number | undefined) {
		console.log('addSection', position);
	}

	function moveSection(position: number | undefined, direction: number) {
		console.log('moveSection', position, direction);
	}
</script>

<div class="right">
	<button type="button" class="action" onclick={() => addSection(section.position)}>add</button>
	<button type="button" class="action" onclick={() => moveSection(section.position, -1)}>up</button>
	<button type="button" class="action" onclick={() => moveSection(section.position, 1)}>down</button
	>
</div>
<div>
	<strong>{section.newPath}</strong>
</div>
<div><pre><code>{section.diffPatch}</code></pre></div>

<style>
	code {
		background-color: #f4f4f4;
		padding: 0.2rem 0.4rem;
		border-radius: 4px;
	}

	strong {
		font-weight: bold;
	}

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
</style>
