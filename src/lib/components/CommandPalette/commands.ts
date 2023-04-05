import QuickCommit from './QuickCommit.svelte';
import type { Project } from '$lib/projects';
import { GitCommitIcon, IconTerminal, RewindIcon } from '../icons';
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
			}
		}))
		.filter(({ title }) => input.length === 0 || title.toLowerCase().includes(input.toLowerCase()))
});

const actionsGroup = ({ project, input }: { project: Project; input: string }): Group => ({
	title: 'Actions',
	commands: [
		{
			title: 'Quick commit',
			hotkey: 'c',
			action: {
				title: 'Quick commit',
				component: QuickCommit,
				props: { project }
			},
			icon: GitCommitIcon
		},
		{
			title: 'Commit',
			hotkey: 'Shift+c',
			action: {
				href: `/projects/${project.id}/commit/`
			},
			icon: GitCommitIcon
		},
		{
			title: 'Terminal',
			hotkey: 'Shift+t',
			action: {
				href: `/projects/${project?.id}/terminal/`
			},
			icon: IconTerminal
		},
		{
			title: 'Replay History',
			hotkey: 'r',
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
					}
				}))
		  }));

export default (params: { projects: Project[]; project?: Project; input: string }) => {
	const { projects, input, project } = params;
	const groups = [];

	!project && groups.push(goToProjectGroup({ projects, input }));
	project && groups.push(actionsGroup({ project, input }));
	project && groups.push(fileGroup({ project, input }));

	return groups;
};
