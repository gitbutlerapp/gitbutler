<script module lang="ts">
	import Avatar from '$components/avatar/Avatar.svelte';
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
				options: ['small', 'medium', 'large'],
				control: {
					type: 'select'
				}
			}
		}
	});
</script>

<Story name="Playground" />
