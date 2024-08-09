export const load: any = ({ params }) => {
	return {
		projectId: params.projectId,
		branchId: params.branchId,
		changeId: params.changeId
	};
};
