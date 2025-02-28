// eslint-disable-next-line func-style
export const load = ({ params }) => {
	return {
		projectId: params.projectId,
		branchId: params.branchId
	};
};
