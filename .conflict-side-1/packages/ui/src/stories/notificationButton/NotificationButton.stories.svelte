<script module lang="ts">
	import NotificationButton from '$lib/NotificationButton.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Inputs / Notification Button',
		component: NotificationButton,
		args: {
			hasUnread: true,
			onclick: () => {
				// eslint-disable-next-line no-console
				console.log('Button clicked');
			}
		}
	});
</script>

<Story name="Playground" />
