/**
 * @desc Represents the order of series (branches) and changes (commits) in a stack.
 * @property series - The series are ordered from newest to oldest (most recent stacks go first).
 */

export type StackOrder = {
	series: SeriesOrder[];
};
/**
 * @desc Represents the order of changes (commits) in a series (branch).
 * @property name - Unique name of the series (branch). Must already exist in the stack.
 * @property commitIds - This is the desired commit order for the series. Because the commits will be rabased, naturally, the the commit ids will be different after updating. The changes are ordered from newest to oldest (most recent changes go first)
 */

type SeriesOrder = {
	name: string;
	commitIds: string[];
};
