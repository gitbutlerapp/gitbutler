<script lang="ts">
	import DecorativeSplitView from '$components/shared/DecorativeSplitView.svelte';
	import InfoMessage from '$components/shared/InfoMessage.svelte';
	import ProjectSwitcher from '$components/shared/ProjectSwitcher.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';

	interface Props {
		projectId?: string;
		error?: any;
	}

	const { projectId, error = undefined }: Props = $props();
</script>

<DecorativeSplitView img={loadErrorSvg}>
	<div class="problem__container">
		<h2 class="problem__title text-18 text-body text-bold">There was a problem loading the app</h2>

		<InfoMessage filled outlined={false} style="error" icon="info">
			{#snippet content()}
				{error ? error : 'An unknown error occurred'}
			{/snippet}
		</InfoMessage>

		<ProjectSwitcher {projectId} />
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
