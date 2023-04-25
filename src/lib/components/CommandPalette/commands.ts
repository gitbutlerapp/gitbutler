import { type Project, git } from '$lib/api';
import { open } from '@tauri-apps/api/dialog';
import { toasts } from '$lib';
import {
	IconGitCommit,
	IconFile,
	IconFeedback,
	IconProject,
	IconTerminal,
	IconSettings,
	IconAdjustmentsHorizontal,
	IconDiscord,
	IconSearch,
	IconRewind
} from '../icons';
import type { SvelteComponent } from 'svelte';

type ActionLink = {
	href: string;
};

type ActionRun = () => void;

interface Newable<ReturnType> {
	new (...args: any[]): ReturnType;
}

export type Action = ActionLink | Group | ActionRun;

export namespace Action {
	export const isLink = (action: Action): action is ActionLink => 'href' in action;
	export const isExternalLink = (action: Action): action is ActionLink =>
		isLink(action) && (action.href.startsWith('http') || action.href.startsWith('mailto'));
	export const isGroup = (action: Action): action is Group => 'commands' in action;
	export const isRun = (action: Action): action is ActionRun => typeof action === 'function';
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

const projectsGroup = ({
	addProject,
	projects,
	input
}: {
	addProject: (params: { path: string }) => Promise<Project>;
	projects: Project[];
	input: string;
}): Group => ({
	title: 'Projects',
	commands: [
		{
			title: 'New project...',
			hotkey: 'Meta+Shift+N',
			icon: IconProject,
			action: async () => {
				const selectedPath = await open({
					directory: true,
					recursive: true
				});
				if (selectedPath === null) return;
				if (Array.isArray(selectedPath) && selectedPath.length !== 1) return;
				const projectPath = Array.isArray(selectedPath) ? selectedPath[0] : selectedPath;

				try {
					addProject({ path: projectPath });
				} catch (e: any) {
					toasts.error(e.message);
				}
			}
		},
		...projects
			.filter(
				({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase())
			)
			.map((project, i) => ({
				title: project.title,
				hotkey: `Meta+${i + 1}`,
				action: {
					href: `/projects/${project.id}/`
				},
				icon: IconProject
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

const navigateGroup = ({ project, input }: { project?: Project; input: string }): Group => ({
	title: 'Navigate',
	commands: [
		...(project
			? [
					{
						title: 'Commit',
						hotkey: 'Meta+Shift+C',
						action: {
							href: `/projects/${project.id}/commit/`
						},
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
						title: 'Terminal',
						hotkey: 'Meta+T',
						action: {
							href: `/projects/${project?.id}/terminal/`
						},
						icon: IconTerminal
					},
					{
						title: 'Project settings',
						hotkey: 'Meta+Shift+,',
						action: {
							href: `/projects/${project?.id}/settings/`
						},
						icon: IconSettings
					}
			  ]
			: []),
		{
			title: 'Settings',
			hotkey: 'Meta+,',
			action: {
				href: '/users/'
			},
			icon: IconAdjustmentsHorizontal
		}
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
		: git.matchFiles({ projectId: project.id, matchPattern: input }).then((files) => ({
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

export default (params: {
	addProject: (params: { path: string }) => Promise<Project>;
	projects: Project[];
	project?: Project;
	input: string;
}) => {
	const { addProject, projects, input, project } = params;
	const groups = [];

	groups.push(navigateGroup({ project, input }));
	!project && groups.push(projectsGroup({ addProject, projects, input }));
	project && groups.push(fileGroup({ project, input }));
	groups.push(supportGroup({ input }));

	return groups;
};
