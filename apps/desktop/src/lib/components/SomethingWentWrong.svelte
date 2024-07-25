<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import Icon from '$lib/shared/Icon.svelte';

	export let error: any = undefined;
</script>

<DecorativeSplitView img={loadErrorSvg}>
	<div class="problem" data-tauri-drag-region>
		<p class="problem__title text-base-body-18 text-bold" data-tauri-drag-region>
			There was a problem loading the app
		</p>

		<div class="problem__error text-base-body-12">
			<Icon name="error" color="error" />
			{error ? error : 'An unknown error occurred'}
		</div>

		<div class="problem__switcher">
			<ProjectSwitcher />
		</div>
	</div>
</DecorativeSplitView>

<style lang="postcss">
	.problem__title {
		color: var(--clr-scale-ntrl-30);
		margin-bottom: 12px;
	}

	.problem__switcher {
		text-align: right;
		margin-top: 24px;
	}

	.problem__error {
		display: flex;
		color: var(--clr-scale-ntrl-0);
		gap: 12px;
		padding: 20px;
		background-color: var(--clr-theme-err-bg);
		border-radius: var(--radius-m);
		margin-bottom: 12px;
	}
</style>
