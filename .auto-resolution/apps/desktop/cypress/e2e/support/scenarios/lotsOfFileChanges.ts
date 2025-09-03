import MockBackend from '../mock/backend';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange
} from '../mock/changes';
import type { TreeChange } from '$lib/hunks/change';

const FILE_PATHS = [
	'fileA.txt',
	'fileB.txt',
	'dir1/fileA.txt',
	'dir1/fileB.txt',
	'dir1/fileC.txt',
	'dir1/fileD.txt',
	'dir1/fileE.txt',
	'dir1/fileF.txt',
	'dir1/fileG.txt',
	'dir1/fileH.txt',
	'dir1/fileI.txt',
	'dir1/fileJ.txt',
	'dir1/fileK.txt',
	'dir2/fileE.txt',
	'dir2/fileF.txt',
	'dir1/subdir1/fileG.txt',
	'dir1/subdir1/fileH.txt',
	'dir2/subdir2/fileI.txt',
	'dir2/subdir2/fileJ.txt',
	'dir3/fileK.txt',
	'dir3/fileL.txt',
	'dir3/subdir3/fileM.txt',
	'dir3/subdir3/fileN.txt',
	'dir4/fileO.txt',
	'dir4/fileP.txt',
	'dir4/subdir4/fileQ.txt',
	'dir4/subdir4/fileR.txt',
	'dir5/fileS.txt',
	'dir5/fileT.txt',
	'dir5/subdir5/fileU.txt',
	'dir5/subdir5/fileV.txt',
	'dir1/subdir1/deep/fileW.txt',
	'dir2/subdir2/deep/fileX.txt',
	'dir3/subdir3/deep/fileY.txt',
	'dir4/subdir4/deep/fileZ.txt',
	'dir5/subdir5/deep/fileAA.txt',
	'dir1/subdir1/deep/deeper/fileAB.txt',
	'dir2/subdir2/deep/deeper/fileAC.txt',
	'dir3/subdir3/deep/deeper/fileAD.txt',
	'dir4/subdir4/deep/deeper/fileAE.txt',
	'dir5/subdir5/deep/deeper/fileAF.txt',
	'dir1/subdir1/deep/deeper/deepest/fileAG.txt',
	'dir2/subdir2/deep/deeper/deepest/fileAH.txt',
	'dir3/subdir3/deep/deeper/deepest/fileAI.txt',
	'dir4/subdir4/deep/deeper/deepest/fileAJ.txt',
	'dir5/subdir5/deep/deeper/deepest/fileAK.txt',
	'dir6/fileAL.txt',
	'dir6/subdir6/fileAM.txt',
	'dir6/subdir6/deep/fileAN.txt',
	'dir6/subdir6/deep/deeper/fileAO.txt',
	'dir6/subdir6/deep/deeper/deepest/fileAP.txt',
	'dir7/fileAQ.txt',
	'dir7/subdir7/fileAR.txt',
	'dir7/subdir7/deep/fileAS.txt',
	'dir7/subdir7/deep/deeper/fileAT.txt',
	'dir7/subdir7/deep/deeper/deepest/fileAU.txt',
	'dir8/fileAV.txt',
	'dir8/subdir8/fileAW.txt',
	'dir8/subdir8/deep/fileAX.txt',
	'dir8/subdir8/deep/deeper/fileAY.txt',
	'dir8/subdir8/deep/deeper/deepest/fileAZ.txt'
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
 * Mock backend for a scenario with a lot of uncommitted file changes.
 */
export default class LotsOfFileChanges extends MockBackend {
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
