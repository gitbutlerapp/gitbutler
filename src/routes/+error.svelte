<script lang="ts">
	import { page } from '$app/stores';
	import { log } from '$lib';

	$: if ($page.error) {
		log.error($page.url, $page.error);
	} else {
		log.error($page.url, $page.status);
	}

	$: message = $page.error
		? $page.error.message
		: $page.status === 404
		? 'Page not found'
		: 'Something went wrong';
</script>

<div class="flex flex-1 h-full">
	<h1 class="m-auto text-xl">
		{message} :(
	</h1>
</div>
