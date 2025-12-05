<script lang="ts">
	import { copyCSS } from '../utils/export';
	import { Button } from '@gitbutler/ui';

	interface Props {
		currentColors: Record<string, Record<number, string>>;
	}

	let { currentColors }: Props = $props();

	async function handleCopyCSS() {
		await copyCSS(currentColors);
	}

	async function handleCopyJSON() {
		const json = JSON.stringify(currentColors, null, 2);
		await navigator.clipboard.writeText(json);
	}
</script>

<div class="export-buttons">
	<Button onclick={handleCopyCSS} kind="outline">Copy CSS</Button>
	<Button onclick={handleCopyJSON} kind="outline">Copy JSON</Button>
</div>

<style>
	.export-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
</style>
