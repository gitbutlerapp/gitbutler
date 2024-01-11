<script lang="ts">
	export let href: string;
	import Icon from '$lib/icons/Icon.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let iconOpacity: string = '0.5';
	export let icon: keyof typeof iconsJson;
</script>

<a class="link" target="_blank" {href}>
	<Icon name={icon} opacity={iconOpacity} />
	<slot />
</a>

<style lang="postcss">
	.link {
		display: flex;
		align-items: center;
		gap: var(--space-12);
	}
</style>
