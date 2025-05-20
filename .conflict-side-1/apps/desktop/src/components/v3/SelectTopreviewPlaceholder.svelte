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
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;
		padding: 40px 0;
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
