<script module lang="ts">
	import Avatar from '$lib/avatar/Avatar.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Avatar / Avatar',
		component: Avatar,
		args: {
			srcUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c',
			tooltip: 'The avatar of bob',
			size: 'medium'
		},
		argTypes: {
			size: {
				control: {
					type: 'select',
					options: ['small', 'medium', 'large']
				}
			}
		}
	});
</script>

<Story name="Playground" />
