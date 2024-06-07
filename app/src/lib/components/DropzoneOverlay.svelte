<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';

	let className = '';
	export { className as class };
	export let label = 'Drop here';
	export let radius: 's' | 'm' | 'l' | undefined = 'm';
</script>

<div
	class="{className} dropzone"
	class:small={radius === 's'}
	class:medium={radius === 'm'}
	class:large={radius === 'l'}
>
	<div class="dropzone-wrapper">
		<div class="dropzone-content">
			<Icon name="new-file-small-filled" />
			<span class="text-base-13">{label}</span>
		</div>
	</div>
</div>

<style lang="post-css">
	@layer dropzone {
		.dropzone {
			user-select: none;
			display: none;
			z-index: var(--z-lifted);
			position: absolute;
			width: 100%;
			height: 100%;

			align-items: center;
			justify-content: center;

			background-color: oklch(from var(--clr-scale-pop-70) l c h / 0.1);

			outline-color: var(--clr-scale-pop-40);
			outline-style: dashed;
			outline-width: 1px;
			outline-offset: -10px;
			backdrop-filter: blur(10px);
		}

		.dropzone-wrapper {
			display: flex;
			flex-direction: column;
			align-items: center;
			justify-content: center;
			width: 100%;
			height: 100%;
		}

		.dropzone-content {
			display: flex;
			align-items: center;
			gap: 6px;
			color: var(--clr-scale-pop-40);
		}

		.small {
			border-radius: var(--radius-s);
		}

		.medium {
			border-radius: var(--radius-m);
		}

		.large {
			border-radius: var(--radius-l);
		}
	}
</style>
