export type AddRemoteParams = {
	projectId: string;
	name: string;
	url: string;
};

export function isAddRemoteParams(params: unknown): params is AddRemoteParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		'name' in params &&
		'url' in params &&
		typeof (params as AddRemoteParams).projectId === 'string' &&
		typeof (params as AddRemoteParams).name === 'string' &&
		typeof (params as AddRemoteParams).url === 'string'
	);
}
