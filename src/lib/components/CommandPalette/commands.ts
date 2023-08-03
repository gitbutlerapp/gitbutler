import type { Project } from '$lib/api/ipc/projects';
import { matchFiles } from '$lib/api/git';
import * as events from '$lib/events';
import {
	IconGitCommit,
	IconFile,
	IconFeedback,
	IconTerminal,
	IconSettings,
	IconDiscord,
	IconSearch,
	IconRewind,
	IconBookmark
} from '$lib/icons';
import type { SvelteComponent } from 'svelte';
import { page } from '$app/stores';
import { get } from '@square/svelte-store';

type ActionLink = {
	href: string;
};

type ActionRun = () => void;

interface Newable<ReturnType> {
	new (...args: any[]): ReturnType;
}

export type Action = ActionLink | Group | ActionRun;

export namespace Action {
	export function isLink(action: Action): action is ActionLink {
		return 'href' in action;
	}
	export function isExternalLink(action: Action): action is ActionLink {
		return isLink(action) && (action.href.startsWith('http') || action.href.startsWith('mailto'));
	}
	export function isGroup(action: Action): action is Group {
		return 'commands' in action;
	}
	export function isRun(action: Action): action is ActionRun {
		return typeof action === 'function';
	}
}

export type Command = {
	title: string;
	hotkey?: string;
	action: Action;
	icon?: Newable<SvelteComponent>;
};

export type Group = {
	title: string;
	description?: string;
	commands: Command[];
};

const projectsGroup = ({ projects, input }: { projects: Project[]; input: string }): Group => ({
	title: 'Projects',
	commands: [
		{
			title: 'New project...',
			hotkey: 'Meta+Shift+N',
			icon: IconFile,
			action: () => events.emit('openNewProjectModal')
		},
		...projects
			.filter(
				({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase())
			)
			.map((project) => ({
				title: project.title,
				action: {
					href: `/projects/${project.id}/`
				},
				icon: IconFile
			})),
		{
			title: 'Search all repositories',
			action: {
				href: '/'
			},
			icon: IconSearch
		}
	].filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

const commandsGroup = ({ project, input }: { project?: Project; input: string }): Group => ({
	title: 'Commands',
	commands: [
		...(project && get(page).params.date
			? [
					{
						title: 'Bookmark',
						hotkey: 'Meta+Shift+D',
						action: () => events.emit('openBookmarkModal'),
						icon: IconBookmark
					},
					{
						title: 'Quick Bookmark',
						hotkey: 'D',
						action: () => events.emit('createBookmark', { projectId: project.id }),
						icon: IconBookmark
					}
			  ]
			: []),
		...(project
			? [
					{
						title: 'Quick commits...',
						hotkey: 'C',
						action: () => events.emit('openQuickCommitModal'),
						icon: IconGitCommit
					},
					{
						title: 'Replay',
						hotkey: 'Meta+R',
						action: {
							href: `/projects/${project.id}/player/`
						},
						icon: IconRewind
					},
					{
						title: 'Project settings',
						hotkey: 'Meta+Shift+,',
						action: {
							href: `/projects/${project.id}/settings/`
						},
						icon: IconSettings
					}
			  ]
			: [])
	].filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

const navigateGroup = ({ project, input }: { project?: Project; input: string }): Group => ({
	title: 'Navigate',
	commands: [
		...(project
			? [
					{
						title: 'Commits',
						hotkey: 'Meta+Shift+C',
						action: {
							href: `/projects/${project.id}/commit/`
						},
						icon: IconGitCommit
					},
					{
						title: 'Terminal',
						hotkey: 'Meta+T',
						action: {
							href: `/projects/${project.id}/terminal/`
						},
						icon: IconTerminal
					},
					{
						title: 'Project settings',
						hotkey: 'Meta+Shift+,',
						action: {
							href: `/projects/${project.id}/settings/`
						},
						icon: IconSettings
					}
			  ]
			: [])
	].filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

const fileGroup = ({
	project,
	input
}: {
	project: Project;
	input: string;
}): Group | Promise<Group> =>
	input.length === 0
		? {
				title: 'Files',
				description: 'type part of a file name',
				commands: []
		  }
		: matchFiles({ projectId: project.id, matchPattern: input }).then((files) => ({
				title: 'Files',
				description: files.length === 0 ? `no files containing '${input}'` : '',
				commands: files.map((file) => ({
					title: file,
					action: {
						href: '/'
					},
					icon: IconFile
				}))
		  }));

const supportGroup = ({ input }: { input: string }): Group => ({
	title: 'Help & Support',
	commands: [
		{
			title: 'Documentation',
			action: {
				href: `https://docs.gitbutler.com`
			},
			icon: IconFile
		},
		{
			title: 'Discord',
			action: {
				href: `https://discord.gg/MmFkmaJ42D`
			},
			icon: IconDiscord
		},
		{
			title: 'Send feedback',
			action: {
				href: 'mailto:hello@gitbutler.com'
			},
			icon: IconFeedback
		}
	].filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

export default (params: { projects: Project[]; project?: Project; input: string }) => {
	const { projects, input, project } = params;
	const groups = [];

	groups.push(commandsGroup({ project, input }));
	groups.push(navigateGroup({ project, input }));
	!project && groups.push(projectsGroup({ projects, input }));
	project && groups.push(fileGroup({ project, input }));
	groups.push(supportGroup({ input }));

	return groups;
};
