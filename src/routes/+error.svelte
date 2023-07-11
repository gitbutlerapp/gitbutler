<script lang="ts">
	import { page } from '$app/stores';

	$: if ($page.error) {
		console.error($page.url, $page.error);
	} else {
		console.log($page.url, $page.status);
	}

	$: message = $page.error
		? $page.error.message
		: $page.status === 404
		? 'Page not found'
		: 'Something went wrong';
</script>

<div class="flex h-full flex-1">
	<h1 class="m-auto text-2xl">
		{message} :(
	</h1>
</div>
