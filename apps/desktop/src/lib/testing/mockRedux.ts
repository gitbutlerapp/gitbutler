import { QueryStatus } from '@reduxjs/toolkit/query';

export function mockReduxFulfilled(data: unknown) {
	return {
		data,
		error: null,
		status: QueryStatus.fulfilled,
		isError: false,
		isLoading: false,
		isSuccess: true
	};
}

export function mockReduxPending() {
	return {
		data: undefined,
		error: null,
		status: QueryStatus.pending,
		isError: false,
		isLoading: true,
		isSuccess: false
	};
}
