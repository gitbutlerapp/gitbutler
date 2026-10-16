<script lang="ts" module>
	export interface Props {
		text: string;
		prefix?: string;
		onclick?: () => void;
		class?: string;
		hideIcon?: boolean;
	}
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";

	const { text, prefix, onclick, class: className, hideIcon = false }: Props = $props();
</script>

<button type="button" class="copy-btn underline-dotted {className}" {onclick}>
	<span>
		{#if prefix}{prefix}
		{/if}{text}
	</span>
	{#if !hideIcon}
		<Icon name="copy" size={14} />
	{/if}
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
