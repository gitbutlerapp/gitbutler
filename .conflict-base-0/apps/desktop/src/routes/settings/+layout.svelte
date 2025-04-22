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
		user-select: none;
		width: 100%;
		height: 100%;
		flex: 1;
		background-color: var(--clr-bg-2);
	}

	.page-view__content {
		padding: 32px 32px 40px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 640px;
		width: 100%;
		min-height: 100vh;
		margin: auto;
	}
</style>
