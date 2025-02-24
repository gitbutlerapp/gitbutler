<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';

	interface Props {
		label?: string;
		content: string;
		marginBottom?: string;
		maxHeight?: string;
	}

	let { label, content, marginBottom, maxHeight = '20lh' }: Props = $props();
	let copied = $state(false);

	function handleCopy() {
		copyToClipboard(content);
		copied = true;
		setTimeout(() => {
			copied = false;
		}, 2000);
	}
</script>

<div class="codeblock-wrapper" style="margin-bottom: {marginBottom}">
	<button type="button" class="codeblock__copy-btn" onclick={handleCopy}>
		<Icon name={copied ? 'tick' : 'copy-small'} />
	</button>

	<div class="codeblock scrollbar" style="max-height: {maxHeight}">
		{#if label}
			<div class="codeblock-label">
				{label}
			</div>
		{/if}

		<div class="codeblock-content text-body">
			{content}
		</div>
	</div>
</div>

<style lang="postcss">
	.codeblock-wrapper {
		position: relative;

		&:hover .codeblock__copy-btn {
			opacity: 1;
		}
	}

	.codeblock {
		display: flex;
		flex-direction: column;
		padding: 12px;
		overflow: auto;
		gap: 8px;
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.codeblock-label {
		color: var(--clr-text-2);
		font-size: 11px;
		line-height: 1.2;
		font-family: var(--font-mono);
		user-select: none;
	}

	.codeblock-content {
		font-size: 12px;
		font-family: var(--font-mono);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.codeblock__copy-btn {
		display: flex;
		position: absolute;
		top: 8px;
		right: 8px;
		padding: 3px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		/* box-shadow: var(--fx-shadow-s); */
		color: var(--clr-text-3);
		opacity: 0;
		transition:
			color var(--transition-fast),
			opacity var(--transition-fast);

		&:hover {
			color: var(--clr-text-2);
		}
	}
</style>
