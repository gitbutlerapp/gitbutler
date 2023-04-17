import type QuickCommit from './QuickCommit.svelte';
import type { Project } from '$lib/projects';
import { GitCommitIcon, IconFile, IconProject, IconTerminal, RewindIcon, FileIcon } from '../icons';
import { matchFiles } from '$lib/git';
import type { SvelteComponent, SvelteComponentTyped } from 'svelte';
import { format, startOfISOWeek, startOfMonth, subDays, subMonths, subWeeks } from 'date-fns';

type ActionLink = {
	href: string;
};

interface Newable<ReturnType> {
	new (...args: any[]): ReturnType;
}

type ComponentProps<T extends SvelteComponentTyped> = T extends SvelteComponentTyped<infer R>
	? R
	: unknown;

export type ActionComponent<Component extends SvelteComponentTyped> = {
	title: string;
	component: Newable<Component>;
	props: ComponentProps<QuickCommit>;
};

export type Action = ActionLink | ActionComponent<QuickCommit> | Group;

export namespace Action {
	export const isLink = (action: Action): action is ActionLink => 'href' in action;
	export const isComponent = (action: Action): action is ActionComponent<any> =>
		'component' in action;
	export const isGroup = (action: Action): action is Group => 'commands' in action;
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

const goToProjectGroup = ({ projects, input }: { projects: Project[]; input: string }): Group => ({
	title: 'Go to project',
	commands: projects
		.map((project, index) => ({
			title: project.title,
			hotkey: `${index + 1}`,
			action: {
				href: `/projects/${project.id}/`
			},
			icon: IconProject
		}))
		.filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

const actionsGroup = ({ project, input }: { project: Project; input: string }): Group => ({
	title: 'Actions',
	commands: [
		{
			title: 'Commit',
			hotkey: 'Shift+C',
			action: {
				href: `/projects/${project.id}/commit/`
			},
			icon: GitCommitIcon
		},
		{
			title: 'Terminal',
			hotkey: 'Shift+T',
			action: {
				href: `/projects/${project?.id}/terminal/`
			},
			icon: IconTerminal
		},
		{
			title: 'Replay History',
			action: {
				title: 'Replay working history',
				commands: [
					{
						title: 'Eralier today',
						icon: RewindIcon,
						hotkey: '1',
						action: {
							href: `/projects/${project.id}/player/${format(new Date(), 'yyyy-MM-dd')}/`
						}
					},
					{
						title: 'Yesterday',
						icon: RewindIcon,
						hotkey: '2',
						action: {
							href: `/projects/${project.id}/player/${format(
								subDays(new Date(), 1),
								'yyyy-MM-dd'
							)}/`
						}
					},
					{
						title: 'The day before yesterday',
						icon: RewindIcon,
						hotkey: '3',
						action: {
							href: `/projects/${project.id}/player/${format(
								subDays(new Date(), 2),
								'yyyy-MM-dd'
							)}/`
						}
					},
					{
						title: 'The beginning of last week',
						icon: RewindIcon,
						hotkey: '4',
						action: {
							href: `/projects/${project.id}/player/${format(
								startOfISOWeek(subWeeks(new Date(), 1)),
								'yyyy-MM-dd'
							)}/`
						}
					},
					{
						title: 'The beginning of last month',
						icon: RewindIcon,
						hotkey: '5',
						action: {
							href: `/projects/${project.id}/player/${format(
								startOfMonth(subMonths(new Date(), 1)),
								'yyyy-MM-dd'
							)}/`
						}
					}
				]
			},
			icon: RewindIcon
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
			icon: FileIcon
		},
		{
			title: 'Discord',
			action: {
				href: `https://discord.gg/MmFkmaJ42D`
			},
			icon: GitCommitIcon
		}
	].filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

export default (params: { projects: Project[]; project?: Project; input: string }) => {
	const { projects, input, project } = params;
	const groups = [];

	!project && groups.push(goToProjectGroup({ projects, input }));
	project && groups.push(actionsGroup({ project, input }));
	project && groups.push(fileGroup({ project, input }));
	groups.push(supportGroup({ input }));

	return groups;
};
