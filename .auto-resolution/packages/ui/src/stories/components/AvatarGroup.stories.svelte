<script module lang="ts">
	import AvatarGroup from '$components/avatar/AvatarGroup.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Avatar / AvatarGroup',
		component: AvatarGroup,
		args: {
			avatars: [
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/76307?s=80&v=4',
					name: 'Sebastian Markbåge'
				},
				{
					srcUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c',
					name: 'Bestest hamster'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/869934?s=80&v=4',
					name: 'Benjamin den Boer'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/869934?s=80&v=4',
					name: 'Benjamin den Boer'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/869934?s=80&v=4',
					name: 'Benjamin den Boer'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/869934?s=80&v=4',
					name: 'Benjamin den Boer'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/14818017?s=64&v=4',
					name: 'Paperstick'
				},
				{
					srcUrl: 'https://avatars.githubusercontent.com/u/11708259?s=64&v=4',
					name: 'Andy Hook'
				}
			],
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
