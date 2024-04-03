<script lang="ts">
	import DecorativeSplitView from './DecorativeSplitView.svelte';
	import ProjectSwitcher from './ProjectSwitcher.svelte';
	import loadErrorSvg from '$lib/assets/illustrations/load-error.svg?raw';
	import Icon from '$lib/components/Icon.svelte';

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
		margin-bottom: var(--size-12);
	}

	.problem__switcher {
		text-align: right;
		margin-top: var(--size-24);
	}

	.problem__error {
		display: flex;
		color: var(--clr-scale-ntrl-0);
		gap: var(--size-12);
		padding: var(--size-20);
		background-color: var(--clr-theme-err-container);
		border-radius: var(--radius-m);
		margin-bottom: var(--size-12);
	}

	.remove-project-btn {
		display: flex;
		justify-content: flex-end;
		padding-bottom: var(--size-24);
		border-bottom: 1px dashed var(--clr-scale-ntrl-60);
	}
</style>
