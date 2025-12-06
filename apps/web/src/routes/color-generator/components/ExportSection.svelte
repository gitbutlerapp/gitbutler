<script lang="ts">
	import { copyCSS, copyJSON } from '../utils/export';
	import { Button } from '@gitbutler/ui';

	interface Props {
		currentColors: Record<string, Record<number, string>>;
	}

	let { currentColors }: Props = $props();

	let cssCopied = $state(false);
	let jsonCopied = $state(false);

	async function handleCopyCSS() {
		await copyCSS(currentColors);
		cssCopied = true;
		setTimeout(() => {
			cssCopied = false;
		}, 2000);
	}

	async function handleCopyJSON() {
		await copyJSON(currentColors);
		jsonCopied = true;
		setTimeout(() => {
			jsonCopied = false;
		}, 2000);
	}
</script>

<div class="export-buttons">
	<Button onclick={handleCopyCSS} kind="solid" icon={cssCopied ? 'tick-small' : 'copy-small'}>
		{cssCopied ? 'Copied' : 'Copy CSS'}
	</Button>
	<Button onclick={handleCopyJSON} kind="outline" icon={jsonCopied ? 'tick-small' : undefined}>
		{jsonCopied ? 'Copied' : 'Copy JSON'}
	</Button>
</div>

<style>
	.export-buttons {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}
</style>
