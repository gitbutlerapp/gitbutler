<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Sidebar from '$components/Sidebar.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		children?: Snippet;
	}

	const { children }: Props = $props();
</script>

<section class="profile-page">
	<Sidebar />

	<section class="page-view">
		<ConfigurableScrollableContainer>
			<div class="page-view__content">
				{@render children?.()}
			</div>
		</ConfigurableScrollableContainer>
	</section>
</section>

<style lang="postcss">
	.profile-page {
		display: flex;
		width: 100%;
	}

	/* PAGE VIEW */
	.page-view {
		flex: 1;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-2);
	}

	.page-view__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 640px;
		margin: 0 auto;
		padding: 32px 32px 40px;
		gap: 16px;
	}
</style>
