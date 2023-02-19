export const toHumanBranchName = (branch: string) => {
    return branch.replace("refs/heads/", "");
};
