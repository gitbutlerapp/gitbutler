<script lang="ts">
	import ProfileSIdebar from '$lib/components/settings/ProfileSIdebar.svelte';
</script>

<section class="profile-page">
	<ProfileSIdebar />
	<slot />
</section>

<style lang="postcss">
	.profile-page {
		display: flex;
		width: 100%;
	}
</style>