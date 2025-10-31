<script lang="ts">
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { env } from '$env/dynamic/public';

	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);

	if ($user) {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login?callback=${window.location.href}`;
	}
</script>
