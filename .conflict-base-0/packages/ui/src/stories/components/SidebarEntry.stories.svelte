<script module lang="ts">
	import SidebarEntry from '$components/SidebarEntry.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const dummySeries = [
		'feature/add-user-auth',
		'bugfix/fix-login-error',
		'hotfix/update-ssl-cert',
		'feature/improve-dashboard-ui',
		'release/v1.2.0',
		'feature/refactor-api-endpoints',
		'bugfix/remove-duplicate-entries',
		'chore/update-dependencies',
		'feature/add-password-reset',
		'hotfix/correct-typo-in-readme'
	];

	const dummyAvatars = [
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
		}
	];

	const { Story } = defineMeta({
		title: 'Cards / Sidebar Entry',
		component: SidebarEntry
	});
</script>

<Story
	name="Not selected"
	args={{
		series: dummySeries,
		selected: false,
		applied: false,
		pullRequestDetails: {
			title: 'bestest pr',
			draft: false
		},
		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: new Date('2021-09-01T12:00:00Z')
		},
		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},
		remotes: [],
		local: true,
		avatars: dummyAvatars
	}}
/>

<Story
	name="With PR"
	args={{
		series: dummySeries,
		selected: false,
		applied: false,
		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: new Date('2021-09-01T12:00:00Z')
		},
		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},
		remotes: ['origin'],
		local: true,
		pullRequestDetails: {
			title: 'bestest pr',
			draft: false
		},
		avatars: dummyAvatars
	}}
/>

<Story
	name="Applied"
	args={{
		series: dummySeries,
		selected: false,
		applied: true,
		lastCommitDetails: {
			authorName: 'Caleb',
			lastCommitAt: new Date('2021-09-01T12:00:00Z')
		},
		branchDetails: {
			commitCount: 4,
			linesAdded: 35,
			linesRemoved: 64
		},
		remotes: ['origin'],
		local: true,
		pullRequestDetails: {
			title: 'bestest pr',
			draft: true
		},
		avatars: dummyAvatars
	}}
/>
