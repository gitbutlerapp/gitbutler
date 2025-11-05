<script lang="ts">
	import SelectToPreviewSVG from '$lib/assets/illustrations/select-to-preview.svg?raw';
</script>

<div class="select-some">
	<div class="select-some__image">
		{@html SelectToPreviewSVG}
	</div>
	<span class="text-13 select-some__caption">Select a file to preview</span>
</div>

<style lang="postcss">
	.select-some {
		display: flex;
		z-index: 0;
		position: absolute;
		top: 50%;
		left: 50%;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		transform: translate(-50%, -50%);

		&:after {
			z-index: -1;
			position: absolute;
			top: calc(50% - 30px);
			left: 50%;
			width: 520px;
			height: 520px;
			transform: translate(-50%, -50%);
			border-radius: 100%;
			background-color: red;
			background: radial-gradient(var(--clr-bg-2) 10%, oklch(from var(--clr-bg-2) l c h / 0) 50%);
			content: '';
			transition: opacity 0.1s;
		}
	}

	.select-some__image {
		width: 100%;
		max-width: 222px;
	}

	.select-some__caption {
		margin-top: 28px;
		color: var(--clr-text-2);
		opacity: 0.6;
	}
</style>
