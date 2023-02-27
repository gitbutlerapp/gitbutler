export const toHumanBranchName = (branch: string | undefined) =>
	branch ? branch.replace('refs/heads/', '') : 'master';
