export type ActionLink = { href: string };
export type ActionInPalette = { action: () => void }; // todo
export type Action = ActionLink | ActionInPalette;

export namespace Action {
	export const isLink = (action: Action): action is ActionLink => 'href' in action;
	export const isActionInPalette = (action: Action): action is ActionInPalette => 'todo' in action;
}

export type Command = {
	title: string;
	description: string;
	// icon: string;
	action: Action;
	selected: boolean;
	visible: boolean;
};
export type CommandGroup = {
	name: string;
	visible: boolean;
	commands: Command[];
};

export const firstVisibleCommand = (commandGroups: CommandGroup[]): [number, number] => {
	const firstVisibleGroup = commandGroups.findIndex((group) => group.visible);
	if (firstVisibleGroup === -1) {
		return [0, 0];
	}
	const firstVisibleCommand = commandGroups[firstVisibleGroup]?.commands.findIndex(
		(command) => command.visible
	);
	if (firstVisibleCommand === -1) {
		return [0, 0];
	}
	return [firstVisibleGroup, firstVisibleCommand];
};

export const nextCommand = (
	commandGroups: CommandGroup[],
	selection: [number, number]
): [number, number] => {
	const { commands } = commandGroups[selection[0]];
	const nextVisibleCommandIndex = commands
		.slice(selection[1] + 1)
		.findIndex((command) => command.visible);

	if (nextVisibleCommandIndex !== -1) {
		return [selection[0], selection[1] + 1 + nextVisibleCommandIndex];
	}

	const nextVisibleGroupIndex = commandGroups
		.slice(selection[0] + 1)
		.findIndex((group) => group.visible);

	if (nextVisibleGroupIndex !== -1) {
		const { commands } = commandGroups[selection[0] + 1 + nextVisibleGroupIndex];
		const nextVisibleCommandIndex = commands.findIndex((command) => command.visible);
		return [selection[0] + 1 + nextVisibleGroupIndex, nextVisibleCommandIndex];
	}

	return [0, 0];
};

export const previousCommand = (
	commandGroups: CommandGroup[],
	selection: [number, number]
): [number, number] => {
	const { commands } = commandGroups[selection[0]];
	const previousVisibleCommandIndex = commands
		.slice(0, selection[1])
		.reverse()
		.findIndex((command) => command.visible);

	if (previousVisibleCommandIndex !== -1) {
		return [selection[0], selection[1] - 1 - previousVisibleCommandIndex];
	}

	const previousVisibleGroupIndex = commandGroups
		.slice(0, selection[0])
		.reverse()
		.findIndex((group) => group.visible);

	if (previousVisibleGroupIndex !== -1) {
		const { commands } = commandGroups[selection[0] - 1 - previousVisibleGroupIndex];
		const previousVisibleCommandIndex = commands
			.slice()
			.reverse()
			.findIndex((command) => command.visible);
		return [selection[0] - 1 - previousVisibleGroupIndex, previousVisibleCommandIndex];
	}

	return [0, 0];
};
