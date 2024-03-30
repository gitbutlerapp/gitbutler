<script lang="ts">
	import { page } from '$app/stores';

	$: if ($page.error) {
		console.error($page.url.toString(), $page.error);
	} else {
		console.log($page.url.toString(), $page.status.toString());
	}

	$: message = $page.error
		? $page.error.message
		: $page.status === 404
			? 'Page not found'
			: 'Something went wrong';
</script>

<div class="wrapper">
	<h1 class="text-base-18 text-semibold">
		{message}
	</h1>
</div>

<style>
	.wrapper {
		display: flex;
		flex: 1 1 0%;
		height: 100%;
	}
</style>
