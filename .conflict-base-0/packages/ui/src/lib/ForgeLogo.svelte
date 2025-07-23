<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import gitlabLogoSmallSvg from '$lib/assets/review-badge/gl-logo-small.svg?raw';
	import gitlabLogoSvg from '$lib/assets/review-badge/gl-logo.svg?raw';

	type Props = {
		forgeName: 'github' | 'gitlab' | (string & {});
		small?: boolean;
	};

	const { forgeName, small = false }: Props = $props();
</script>

{#if forgeName === 'github'}
	<Icon name={small ? 'github-small' : 'github'} />
{:else if forgeName === 'gitlab'}
	<div class="forge-logo">
		{#if small}
			{@html gitlabLogoSmallSvg}
		{:else}
			{@html gitlabLogoSvg}
		{/if}
	</div>
{/if}

<style lang="postcss">
	.forge-logo {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
	}
</style>
