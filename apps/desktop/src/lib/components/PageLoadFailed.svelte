<script lang="ts">
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';

	export let error: any = undefined;
</script>

<div class="decorative-split-view">
	<div class="left-side">
		<div class="left-side__content">
			<h2 class="problem__title text-18 text-body text-bold">
				There was a problem loading this page
			</h2>

			<InfoMessage filled outlined={false} style="error" icon="info">
				<svelte:fragment slot="content">
					{error ? error : 'An unknown error occurred'}
				</svelte:fragment>
			</InfoMessage>
		</div>
	</div>
	<div class="right-side">
		<div class="right-side-wrapper">
			<div class="img-wrapper">
				{@html loadErrorSvg}
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.decorative-split-view {
		cursor: default;
		user-select: none;
		display: flex;
		flex-grow: 1;
		background-color: var(--clr-bg-1);
	}
	.right-side {
		display: flex;
		flex-direction: column;
		position: relative;
	}

	.left-side {
		display: grid;
		align-items: center;
		padding: 40px 80px;
		flex: 1.3;
		background-color: var(--clr-bg-1);
		overflow-y: auto;
	}
	.left-side__content {
		display: flex;
		flex-direction: column;
		margin: 0 auto;
		width: 100%;
		max-width: 512px;
	}

	.problem__title {
		color: var(--clr-scale-ntrl-30);
	}
	.img-wrapper {
		flex: 1;
		width: 100%;
		max-width: 440px;
		overflow: hidden;
		padding: 0 24px;
	}
	.right-side-wrapper {
		overflow: hidden;
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		border-radius: 8px;
	}
	.problem__title {
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 12px;
	}
</style>
