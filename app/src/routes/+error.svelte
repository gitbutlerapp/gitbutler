<script lang="ts">
	import SomethingWentWrong from '$lib/components/SomethingWentWrong.svelte';
	import { page } from '$app/stores';

	// eslint-disable-next-line svelte/valid-compile
	$: message = $page.error
		? $page.error.message
		: $page.status === 404
			? 'Page not found'
			: 'Unknown error';
</script>

<SomethingWentWrong error={message} />
