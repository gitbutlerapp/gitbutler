<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';

	interface Props {
		error?: any;
	}

	let { error = undefined }: Props = $props();
</script>

<DecorativeSplitView img={loadErrorSvg}>
	<div class="problem__container">
		<h2 class="problem__title text-18 text-body text-bold">There was a problem loading the app</h2>

		<InfoMessage filled outlined={false} style="error" icon="info">
			{#snippet content()}
				{error ? error : 'An unknown error occurred'}
			{/snippet}
		</InfoMessage>

		<ProjectSwitcher />
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.problem__container {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.problem__title {
		color: var(--clr-scale-ntrl-30);
	}
</style>
