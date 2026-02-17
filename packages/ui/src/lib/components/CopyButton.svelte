<script lang="ts" module>
	export interface Props {
		/**
		 * The text to display and copy
		 */
		text: string;
		/**
		 * Optional prefix to display before the text (e.g., "upstream")
		 */
		prefix?: string;
		/**
		 * Whether to show only the first 7 characters (useful for SHAs)
		 * @default true
		 */
		shortenText?: boolean;
		/**
		 * Callback when the button is clicked
		 */
		onclick?: () => void;
		/**
		 * Additional CSS classes
		 */
		class?: string;
	}
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";

	const { text, prefix, shortenText = true, onclick, class: className }: Props = $props();

	const displayText = $derived(shortenText ? text.substring(0, 7) : text);
</script>

<button type="button" class="copy-btn underline-dotted {className}" {onclick}>
	<span>
		{#if prefix}{prefix}
		{/if}{displayText}
	</span>
	<Icon name="copy-small" />
</button>

<style lang="postcss">
	.copy-btn {
		display: flex;
		align-items: center;
		transition: color var(--transition-fast);

		& span {
			margin-right: 2px;
			text-align: left;
		}
	}
</style>
