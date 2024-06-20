<script lang="ts">
	import Sidebar from '$lib/settings/Sidebar.svelte';
</script>

<section class="profile-page">
	<Sidebar />
	<slot />
</section>

<style lang="postcss">
	.profile-page {
		display: flex;
		width: 100%;
	}
</style>
