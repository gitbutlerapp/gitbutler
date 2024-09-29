<script lang="ts">
	import Sidebar from '$lib/settings/Sidebar.svelte';
	interface Props {
		children?: import('svelte').Snippet;
	}

	let { children }: Props = $props();
</script>

<section class="profile-page">
	<Sidebar />
	{@render children?.()}
</section>

<style lang="postcss">
	.profile-page {
		display: flex;
		width: 100%;
	}
</style>
