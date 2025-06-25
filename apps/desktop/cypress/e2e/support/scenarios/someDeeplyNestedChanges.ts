import MockBackend from '../mock/backend';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange
} from '../mock/changes';
import type { TreeChange } from '$lib/hunks/change';

const FILE_PATHS = [
	'dir1/subdir1/deep/deeper/deepest/fileAG.txt',
	'dir2/subdir2/deep/deeper/deepest/fileAH.txt',
	'dir3/subdir3/deep/deeper/deepest/fileAI.txt',
	'dir4/subdir4/deep/deeper/deepest/fileAJ.txt',
	'dir5/subdir5/deep/deeper/deepest/fileAK.txt'
];

const treeChangeGenerators = [
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange
];

const MOCK_FILE_TREE_CHANGES: TreeChange[] = FILE_PATHS.map((path) => {
	const randomGeneratorIndex = Math.floor(Math.random() * treeChangeGenerators.length);
	const randomGenerator = treeChangeGenerators[randomGeneratorIndex]!;
	return randomGenerator({ path });
});

/**
 * Mock backend for a scenario with only some deeply nested file changes.
 */
export default class SomeDeeplyNestedChanges extends MockBackend {
	constructor() {
		super();

		this.worktreeChanges = {
			changes: MOCK_FILE_TREE_CHANGES,
			ignoredChanges: [],
			assignments: [],
			assignmentsError: null,
			dependencies: {
				diffs: [],
				errors: []
			},
			dependenciesError: null
		};
	}
}
