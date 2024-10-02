import { AUTO_GROUP, groupCommitsByRef } from './commitGroups';
import { DetailedCommit } from './types';
import { expect, test } from 'vitest';

test('group commits correctly by remote ref', () => {
	const commits = [
		{ id: '0' },
		{ id: '1', remoteRef: 'a' },
		{ id: '2', remoteRef: 'b' },
		{ id: '3' },
		{ id: '4' },
		{ id: '5', remoteRef: 'c' },
		{ id: '6' }
	] as DetailedCommit[];

	const groups = groupCommitsByRef(commits);
	expect(groups.length).toEqual(4);

	const [group0, groupA, groupB, groupC] = groups;
	expect(group0?.ref).toEqual(AUTO_GROUP);
	expect(groupA?.ref).toEqual('a');
	expect(groupB?.ref).toEqual('b');
	expect(groupC?.ref).toEqual('c');
	expect(group0?.commits.length).toEqual(1);
	expect(groupA?.commits.length).toEqual(1);
	expect(groupB?.commits.length).toEqual(3);
	expect(groupC?.commits.length).toEqual(2);
});

test('group commits with undefined head ref', () => {
	const commits = [{ id: '1' }, { id: '2', remoteRef: 'b' }] as DetailedCommit[];
	const groups = groupCommitsByRef(commits);
	expect(groups.length).toEqual(2);

	const [groupA, groupB] = groups;
	expect(groupA?.ref).toEqual(AUTO_GROUP);
	expect(groupB?.ref).toEqual('b');
});
