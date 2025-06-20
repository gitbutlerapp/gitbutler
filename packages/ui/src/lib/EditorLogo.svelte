<script lang="ts">
	import cursorLogoSvg from '$lib/assets/cursor.svg?raw';
	import vsCodeInsidersLogoSvg from '$lib/assets/vscode-insiders.svg?raw';
	import vsCodeLogoSvg from '$lib/assets/vscode.svg?raw';

	type Props = {
		name: string;
	};

	const { name }: Props = $props();
	const isCursor = $derived(name.toLowerCase().includes('cursor'));
	const vsCodeKeywords = ['vscode', 'visual studio code'];
	const isVsCode = $derived(vsCodeKeywords.some((keyword) => name.toLowerCase().includes(keyword)));
	const isVsCodeInsiders = $derived(isVsCode && name.toLowerCase().includes('insiders'));
</script>

<div class="editor-logo">
	{#if isCursor}
		{@html cursorLogoSvg}
	{:else if isVsCodeInsiders}
		{@html vsCodeInsidersLogoSvg}
	{:else if isVsCode}
		{@html vsCodeLogoSvg}
	{/if}
</div>

<style lang="postcss">
	.editor-logo {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		padding: 2px;
		border-radius: 8px;
		background-color: var(--clr-core-ntrl-10);
	}
</style>
